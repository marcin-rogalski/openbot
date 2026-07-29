//! Live voice-channel transcription. When a bot joins a voice channel it
//! receives per-speaker decoded PCM (songbird, `DecodeMode::Decode`), segments
//! each speaker's audio on silence, and transcribes each utterance via the
//! model server. On leave it assembles a speaker-labelled meeting transcript +
//! summary (posted to the text channel and, with a Drive tool, indexed).
//!
//! Audio from songbird is 48 kHz stereo interleaved `i16`; we downmix to mono
//! and wrap each utterance in a WAV container for `/audio/transcriptions`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serenity::async_trait;
use songbird::events::context_data::VoiceTick;
use songbird::model::payload::Speaking;
use songbird::{Event, EventContext, EventHandler as VoiceEventHandler};
use tokio::sync::Mutex;

use crate::config::BotConfig;

/// Consecutive silent 20 ms ticks that close an utterance (~0.8 s).
const SILENCE_TICKS: u32 = 40;
/// Ignore utterances shorter than this many mono samples (~0.4 s at 48 kHz).
const MIN_MONO_SAMPLES: usize = 19_200;
/// Force-flush a single utterance once it reaches this many stereo samples
/// (~30 s), so one long talker still gets transcribed in pieces.
const MAX_STEREO_SAMPLES: usize = 48_000 * 2 * 30;
/// songbird decodes to this by default (stereo, interleaved L,R,L,R).
const SAMPLE_RATE: u32 = 48_000;

/// One transcribed utterance, kept in speech order via `seq`.
#[derive(Clone)]
struct Line {
    seq: u64,
    /// Seconds since the meeting started, for a readable timestamp.
    at_secs: u64,
    user_id: Option<u64>,
    text: String,
}

/// Per-speaker accumulating audio (stereo interleaved) + a running silence count.
#[derive(Default)]
struct Buf {
    pcm: Vec<i16>,
    silent: u32,
}

/// Live state for one active meeting (one guild's voice connection).
pub struct Meeting {
    bot: BotConfig,
    started: Instant,
    seq: AtomicU64,
    /// SSRC → Discord user id (learned from speaking-state updates).
    ssrc_user: Mutex<HashMap<u32, u64>>,
    buffers: Mutex<HashMap<u32, Buf>>,
    transcript: Mutex<Vec<Line>>,
}

impl Meeting {
    pub fn new(bot: BotConfig) -> Arc<Self> {
        Arc::new(Self {
            bot,
            started: Instant::now(),
            seq: AtomicU64::new(0),
            ssrc_user: Mutex::new(HashMap::new()),
            buffers: Mutex::new(HashMap::new()),
            transcript: Mutex::new(Vec::new()),
        })
    }

    async fn map_ssrc(&self, ssrc: u32, user_id: u64) {
        self.ssrc_user.lock().await.insert(ssrc, user_id);
    }

    /// Handle one 20 ms tick: append fresh audio for each speaker and flush any
    /// speaker that has now been silent long enough (or overran the cap).
    async fn on_tick(self: &Arc<Self>, tick: &VoiceTick) {
        let mut flush: Vec<(u32, Vec<i16>)> = Vec::new();
        {
            let mut buffers = self.buffers.lock().await;
            for (ssrc, data) in &tick.speaking {
                if let Some(pcm) = &data.decoded_voice {
                    let buf = buffers.entry(*ssrc).or_default();
                    buf.pcm.extend_from_slice(pcm);
                    buf.silent = 0;
                    if buf.pcm.len() >= MAX_STEREO_SAMPLES {
                        flush.push((*ssrc, std::mem::take(&mut buf.pcm)));
                    }
                }
            }
            for ssrc in &tick.silent {
                if let Some(buf) = buffers.get_mut(ssrc) {
                    if buf.pcm.is_empty() {
                        continue;
                    }
                    buf.silent += 1;
                    if buf.silent >= SILENCE_TICKS {
                        let pcm = std::mem::take(&mut buf.pcm);
                        buf.silent = 0;
                        flush.push((*ssrc, pcm));
                    }
                }
            }
        }
        for (ssrc, pcm) in flush {
            self.clone().spawn_transcribe(ssrc, pcm);
        }
    }

    /// Transcribe one utterance off the tick path so it never blocks receiving.
    fn spawn_transcribe(self: Arc<Self>, ssrc: u32, stereo: Vec<i16>) {
        let mono = downmix_mono(&stereo);
        if mono.len() < MIN_MONO_SAMPLES {
            return;
        }
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let at_secs = self.started.elapsed().as_secs();
        tokio::spawn(async move {
            let user_id = self.ssrc_user.lock().await.get(&ssrc).copied();
            let wav = crate::audio::pcm_to_wav(&mono, SAMPLE_RATE);
            if let Some(text) =
                crate::infrastructure::driving::transcription::transcribe_wav_text(&self.bot, wav)
                    .await
            {
                self.transcript.lock().await.push(Line {
                    seq,
                    at_secs,
                    user_id,
                    text: text.trim().to_string(),
                });
            }
        });
    }

    /// Transcribe any audio still buffered (called at meeting end, inline).
    async fn flush_remaining(self: &Arc<Self>) {
        let leftovers: Vec<(u32, Vec<i16>)> = {
            let mut buffers = self.buffers.lock().await;
            buffers
                .iter_mut()
                .filter(|(_, b)| !b.pcm.is_empty())
                .map(|(ssrc, b)| (*ssrc, std::mem::take(&mut b.pcm)))
                .collect()
        };
        for (ssrc, stereo) in leftovers {
            let mono = downmix_mono(&stereo);
            if mono.len() < MIN_MONO_SAMPLES {
                continue;
            }
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            let at_secs = self.started.elapsed().as_secs();
            let user_id = self.ssrc_user.lock().await.get(&ssrc).copied();
            let wav = crate::audio::pcm_to_wav(&mono, SAMPLE_RATE);
            if let Some(text) =
                crate::infrastructure::driving::transcription::transcribe_wav_text(&self.bot, wav)
                    .await
            {
                self.transcript.lock().await.push(Line {
                    seq,
                    at_secs,
                    user_id,
                    text: text.trim().to_string(),
                });
            }
        }
    }

    /// Finalise: flush leftovers, order by speech time, and render the transcript
    /// body plus the set of user ids seen (for name resolution by the caller).
    pub async fn render(self: &Arc<Self>) -> Option<Rendered> {
        self.flush_remaining().await;
        // Give any in-flight utterance tasks a moment to land.
        tokio::time::sleep(std::time::Duration::from_millis(750)).await;

        let mut lines = self.transcript.lock().await.clone();
        if lines.is_empty() {
            return None;
        }
        lines.sort_by_key(|l| l.seq);
        let user_ids: Vec<u64> = {
            let mut ids: Vec<u64> = lines.iter().filter_map(|l| l.user_id).collect();
            ids.sort_unstable();
            ids.dedup();
            ids
        };
        Some(Rendered {
            lines,
            user_ids,
            minutes: self.started.elapsed().as_secs() / 60,
        })
    }
}

/// The ordered transcript plus the ids needing name resolution.
pub struct Rendered {
    lines: Vec<Line>,
    pub user_ids: Vec<u64>,
    pub minutes: u64,
}

impl Rendered {
    /// Build the transcript body, resolving ids to display names via `names`.
    pub fn body(&self, names: &HashMap<u64, String>) -> String {
        self.lines
            .iter()
            .map(|l| {
                let who = l
                    .user_id
                    .and_then(|id| names.get(&id).cloned())
                    .unwrap_or_else(|| "Unknown".to_string());
                format!(
                    "[{:02}:{:02}] {who}: {}",
                    l.at_secs / 60,
                    l.at_secs % 60,
                    l.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// songbird event receiver bound to one meeting: maps SSRCs to users and feeds
/// each tick's audio into the meeting's buffers.
pub struct Receiver {
    meeting: Arc<Meeting>,
}

impl Receiver {
    pub fn new(meeting: Arc<Meeting>) -> Self {
        Self { meeting }
    }
}

#[async_trait]
impl VoiceEventHandler for Receiver {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        match ctx {
            EventContext::SpeakingStateUpdate(Speaking {
                ssrc,
                user_id: Some(uid),
                ..
            }) => {
                self.meeting.map_ssrc(*ssrc, uid.0).await;
            }
            EventContext::VoiceTick(tick) => {
                self.meeting.on_tick(tick).await;
            }
            _ => {}
        }
        None
    }
}

/// Average interleaved stereo into mono.
fn downmix_mono(stereo: &[i16]) -> Vec<i16> {
    stereo
        .chunks_exact(2)
        .map(|c| ((c[0] as i32 + c[1] as i32) / 2) as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_channels() {
        assert_eq!(downmix_mono(&[100i16, 200, -100, 100]), vec![150i16, 0]);
    }
}

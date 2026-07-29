//! Driving adapter for transcription. Shared by every surface that produces a
//! transcript (Discord audio attachments today; the Drive-link tool op and voice
//! meetings as they migrate). Runs the clip usecase and renders the transcript /
//! summary for delivery.

use std::path::{Path, PathBuf};

use crate::compose::transcription::{
    compose_audio_codec, compose_summarizer, compose_transcribe_clip, compose_transcriber,
};
use crate::config::BotConfig;
use crate::domain::transcript::Transcript;

/// Transcribe a single audio clip into a `Transcript`.
pub async fn transcribe_clip(
    bot: &BotConfig,
    bytes: Vec<u8>,
    filename: &str,
    mime: &str,
) -> Result<Transcript, String> {
    compose_transcribe_clip(bot)
        .run(bytes, filename, mime)
        .await
}

/// Transcribe a long recording from a local file: split into WAV chunks, then
/// transcribe each (bounded memory), assembling one absolute-timestamped
/// `Transcript`. `on_chunk(i, n)` reports per-chunk progress. Consumes `source`
/// (and its chunk files) as it goes. Returns the transcript and whether the
/// recording was truncated at `max_chunks`.
pub async fn transcribe_recording(
    bot: &BotConfig,
    source: &Path,
    filename: &str,
    mime: &str,
    chunk_secs: u32,
    max_chunks: usize,
    on_chunk: &(dyn Fn(usize, usize) + Sync),
) -> Result<(Transcript, bool), String> {
    let out_dir = source
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let (chunks, truncated) = compose_audio_codec()
        .split_to_wav_chunks(
            source.to_path_buf(),
            filename.to_string(),
            mime.to_string(),
            chunk_secs,
            max_chunks,
            out_dir,
        )
        .await?;
    let _ = std::fs::remove_file(source); // no longer needed once split

    let transcriber = compose_transcriber(bot);
    let n = chunks.len();
    let mut transcript = Transcript::default();
    for (i, chunk) in chunks.iter().enumerate() {
        on_chunk(i + 1, n);
        let Ok(bytes) = std::fs::read(chunk) else {
            continue;
        };
        if let Ok(segs) = transcriber
            .transcribe(bytes, "chunk.wav", "audio/wav")
            .await
        {
            transcript.append(segs, f64::from(i as u32 * chunk_secs));
        }
        let _ = std::fs::remove_file(chunk);
    }
    Ok((transcript, truncated))
}

/// Summarise a transcript; on failure returns a placeholder note (never errors,
/// so a failed summary doesn't sink the whole transcription).
pub async fn summarize(bot: &BotConfig, plain: &str) -> String {
    compose_summarizer(bot)
        .summarize(plain)
        .await
        .unwrap_or_else(|e| format!("_(summary unavailable: {e})_"))
}

/// Render a transcript as a `[MM:SS] text` document.
pub fn render_timestamped(transcript: &Transcript) -> String {
    transcript
        .segments
        .iter()
        .map(|s| {
            let t = s.start.max(0.0) as u64;
            format!("[{:02}:{:02}] {}", t / 60, t % 60, s.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

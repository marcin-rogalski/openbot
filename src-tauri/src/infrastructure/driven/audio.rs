//! Audio helpers: decode compressed audio/video (mp3/m4a/flac/ogg/mp4/mov …) to
//! mono PCM and wrap PCM in a WAV container. Decoding is pure-Rust (symphonia),
//! so posted or linked media is normalised to WAV before transcription and the
//! model server needs no extra codecs (e.g. no ffmpeg).
//!
//! Two entry points: [`decode_to_wav`] for small in-memory clips (Discord
//! attachments), and [`split_to_wav_chunks`] which streams a large file from disk
//! into fixed-length WAV chunks so long recordings transcribe with bounded memory.

use std::path::{Path, PathBuf};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_OPUS};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Default per-chunk length when splitting long audio for transcription.
pub const CHUNK_SECS: u32 = 300;

/// The decoder for the chosen audio track — symphonia for most codecs, or
/// audiopus for Opus (which symphonia demuxes but can't decode).
enum Dec {
    Symphonia(Box<dyn Decoder>),
    Opus(audiopus::coder::Decoder),
}

/// An opened decoder bound to a media source's audio track.
struct Reader {
    format: Box<dyn FormatReader>,
    dec: Dec,
    track_id: u32,
    sample_rate: u32,
    channels: usize,
}

/// Probe a media source and set up a decoder for its default track.
fn open(source: Box<dyn MediaSource>, filename: &str, mime: &str) -> Option<Reader> {
    let mss = MediaSourceStream::new(source, Default::default());
    let mut hint = Hint::new();
    if let Some((_, ext)) = filename.rsplit_once('.') {
        hint.with_extension(ext);
    }
    let bare = mime.split(';').next().unwrap_or(mime).trim();
    if !bare.is_empty() {
        hint.mime_type(bare);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;
    let format = probed.format;
    // Pick the first track we can build an *audio* decoder for. In video files
    // the default/first track is the video stream (no audio decoder), so trying
    // to make a decoder is the reliable way to find the audio track.
    // Prefer a track we can build a symphonia decoder for (in video files the
    // default track is the video stream); fall back to an Opus track (audiopus).
    let codecs = symphonia::default::get_codecs();
    let mut opus_track: Option<u32> = None;
    for track in format.tracks() {
        let cp = &track.codec_params;
        if let Some(sample_rate) = cp.sample_rate {
            if let Ok(decoder) = codecs.make(cp, &DecoderOptions::default()) {
                let channels = cp.channels.map(|c| c.count()).unwrap_or(1).max(1);
                return Some(Reader {
                    track_id: track.id,
                    sample_rate,
                    channels,
                    dec: Dec::Symphonia(decoder),
                    format,
                });
            }
        }
        if cp.codec == CODEC_TYPE_OPUS && opus_track.is_none() {
            opus_track = Some(track.id);
        }
    }
    if let Some(track_id) = opus_track {
        let decoder = audiopus::coder::Decoder::new(
            audiopus::SampleRate::Hz48000,
            audiopus::Channels::Stereo,
        )
        .ok()?;
        return Some(Reader {
            track_id,
            sample_rate: 48_000, // Opus always decodes to 48 kHz
            channels: 2,
            dec: Dec::Opus(decoder),
            format,
        });
    }
    None
}

/// Average interleaved n-channel samples into mono and append to `mono`.
fn append_downmix(samples: &[i16], channels: usize, mono: &mut Vec<i16>) {
    if channels <= 1 {
        mono.extend_from_slice(samples);
    } else {
        for frame in samples.chunks(channels) {
            let sum: i32 = frame.iter().map(|&s| s as i32).sum();
            mono.push((sum / channels as i32) as i16);
        }
    }
}

/// Decode the next packet, appending its samples (downmixed to mono) to `mono`.
/// Returns `false` at end of stream.
fn next_mono(r: &mut Reader, buf: &mut Option<SampleBuffer<i16>>, mono: &mut Vec<i16>) -> bool {
    loop {
        let packet = match r.format.next_packet() {
            Ok(p) => p,
            Err(_) => return false, // end of stream / read error
        };
        if packet.track_id() != r.track_id {
            continue;
        }
        match &mut r.dec {
            Dec::Symphonia(decoder) => {
                let decoded = match decoder.decode(&packet) {
                    Ok(d) => d,
                    Err(SymphoniaError::DecodeError(_)) => continue, // recoverable
                    Err(_) => return false,
                };
                if buf.is_none() {
                    *buf = Some(SampleBuffer::<i16>::new(
                        decoded.capacity() as u64,
                        *decoded.spec(),
                    ));
                }
                let Some(sb) = buf.as_mut() else {
                    return false;
                };
                sb.copy_interleaved_ref(decoded);
                append_downmix(sb.samples(), r.channels, mono);
                return true;
            }
            Dec::Opus(decoder) => {
                // Each ogg packet is one Opus frame; skip the header packets
                // (OpusHead/OpusTags) and anything that fails to decode.
                let Ok(pkt) = audiopus::packet::Packet::try_from(&packet.data[..]) else {
                    continue;
                };
                // Max Opus frame is 120 ms → 5760 samples/channel (stereo here).
                let mut out = [0i16; 5760 * 2];
                let Ok(signals) = audiopus::MutSignals::try_from(&mut out[..]) else {
                    return false;
                };
                match decoder.decode(Some(pkt), signals, false) {
                    Ok(frames) => {
                        let total = (frames * 2).min(out.len());
                        append_downmix(&out[..total], 2, mono);
                        return true;
                    }
                    Err(_) => continue,
                }
            }
        }
    }
}

/// Decode arbitrary audio bytes to a mono 16-bit WAV (whole thing in memory).
/// Returns `None` if the format/codec isn't supported (e.g. Opus).
pub fn decode_to_wav(bytes: &[u8], filename: &str, mime: &str) -> Option<Vec<u8>> {
    let mut r = open(
        Box::new(std::io::Cursor::new(bytes.to_vec())),
        filename,
        mime,
    )?;
    let mut buf = None;
    let mut mono = Vec::new();
    while next_mono(&mut r, &mut buf, &mut mono) {}
    if mono.is_empty() {
        return None;
    }
    Some(pcm_to_wav(&mono, r.sample_rate))
}

/// Stream-decode a file into fixed-length mono WAV chunks written to `out_dir`,
/// so arbitrarily long recordings can be transcribed with bounded memory.
/// Stops after `max_chunks` (the returned bool is `true` when it was truncated).
pub fn split_to_wav_chunks(
    path: &Path,
    filename: &str,
    mime: &str,
    chunk_secs: u32,
    max_chunks: usize,
    out_dir: &Path,
) -> Result<(Vec<PathBuf>, bool), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open failed: {e}"))?;
    let mut r = open(Box::new(file), filename, mime)
        .ok_or_else(|| "unsupported or unreadable audio".to_string())?;
    let chunk_frames = (r.sample_rate as usize) * (chunk_secs.max(1) as usize);
    let sample_rate = r.sample_rate;

    let mut buf = None;
    let mut mono: Vec<i16> = Vec::new();
    let mut chunks: Vec<PathBuf> = Vec::new();
    let mut truncated = false;

    while next_mono(&mut r, &mut buf, &mut mono) {
        if mono.len() >= chunk_frames {
            let idx = chunks.len();
            chunks.push(write_wav_chunk(&mono, sample_rate, out_dir, idx)?);
            mono.clear();
            if chunks.len() >= max_chunks {
                truncated = true;
                break;
            }
        }
    }
    if !truncated && !mono.is_empty() {
        let idx = chunks.len();
        chunks.push(write_wav_chunk(&mono, sample_rate, out_dir, idx)?);
    }
    if chunks.is_empty() {
        return Err("no audio could be decoded".to_string());
    }
    Ok((chunks, truncated))
}

fn write_wav_chunk(mono: &[i16], sr: u32, out_dir: &Path, idx: usize) -> Result<PathBuf, String> {
    let p = out_dir.join(format!("chunk-{idx:04}.wav"));
    std::fs::write(&p, pcm_to_wav(mono, sr)).map_err(|e| format!("write chunk failed: {e}"))?;
    Ok(p)
}

/// Wrap mono 16-bit PCM in a minimal WAV container.
pub fn pcm_to_wav(mono: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_len = (mono.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // channels = mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in mono {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_valid() {
        let wav = pcm_to_wav(&[0i16, 1, -1], 48_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + 6);
    }

    #[test]
    fn decode_rejects_non_audio() {
        assert!(decode_to_wav(b"not audio at all", "x.mp3", "audio/mpeg").is_none());
    }

    #[test]
    fn split_produces_multiple_chunks() {
        // A 3-second 8 kHz WAV, split at 1 s, should yield ~3 chunks.
        let dir = std::env::temp_dir().join(format!("openbot-split-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("in.wav");
        std::fs::write(&src, pcm_to_wav(&vec![0i16; 8_000 * 3], 8_000)).unwrap();

        let (chunks, truncated) =
            split_to_wav_chunks(&src, "in.wav", "audio/wav", 1, 100, &dir).unwrap();
        assert!(!truncated);
        assert!(
            chunks.len() >= 3,
            "expected ≥3 chunks, got {}",
            chunks.len()
        );
        for c in &chunks {
            assert!(c.exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

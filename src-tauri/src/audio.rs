//! Audio helpers: decode compressed audio (mp3/m4a/flac/ogg/wav) to mono PCM and
//! wrap PCM in a WAV container. Decoding is pure-Rust (symphonia), so posted
//! audio is normalised to WAV before transcription and the model server needs no
//! extra codecs (e.g. no ffmpeg).

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decode arbitrary audio bytes to a mono 16-bit WAV. Returns `None` if the
/// format/codec isn't supported (e.g. Opus) or the stream can't be decoded — the
/// caller can then fall back to sending the original bytes.
pub fn decode_to_wav(bytes: &[u8], filename: &str, mime: &str) -> Option<Vec<u8>> {
    let cursor = std::io::Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    if let Some((_, ext)) = filename.rsplit_once('.') {
        hint.with_extension(ext);
    }
    let bare_mime = mime.split(';').next().unwrap_or(mime).trim();
    if !bare_mime.is_empty() {
        hint.mime_type(bare_mime);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;
    let mut format = probed.format;

    let track = format.default_track()?.clone();
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate?;
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(1)
        .max(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .ok()?;

    let mut mono = Vec::<i16>::new();
    let mut buf: Option<SampleBuffer<i16>> = None;

    // `next_packet` returns Err at end-of-stream, which ends the loop.
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue, // recoverable, skip frame
            Err(_) => break,
        };
        if buf.is_none() {
            buf = Some(SampleBuffer::<i16>::new(
                decoded.capacity() as u64,
                *decoded.spec(),
            ));
        }
        let sb = buf.as_mut()?;
        sb.copy_interleaved_ref(decoded);
        let samples = sb.samples();
        if channels <= 1 {
            mono.extend_from_slice(samples);
        } else {
            for frame in samples.chunks(channels) {
                let sum: i32 = frame.iter().map(|&s| s as i32).sum();
                mono.push((sum / channels as i32) as i16);
            }
        }
    }

    if mono.is_empty() {
        return None;
    }
    Some(pcm_to_wav(&mono, sample_rate))
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
}

//! Driving adapter for transcription. Shared by every surface that produces a
//! transcript (Discord audio attachments today; the Drive-link tool op and voice
//! meetings as they migrate). Runs the clip usecase and renders the transcript /
//! summary for delivery.

use crate::compose::transcription::{compose_summarizer, compose_transcribe_clip};
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

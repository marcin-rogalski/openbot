//! Turn a downloaded file into indexable text: extract, then chunk. Text-ish
//! files and text-based PDFs are supported; OCR / docx are deferred.

/// Extract plain text from raw bytes, or `None` if the type isn't supported yet.
pub fn extract_text(bytes: &[u8], filename: &str, mime: &str) -> Option<String> {
    let ext = filename
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();

    if mime == "application/pdf" || ext == "pdf" {
        return pdf_extract::extract_text_from_mem(bytes)
            .ok()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
    }

    if is_text(mime, &ext) {
        // Lossy so a stray non-UTF-8 byte doesn't drop the whole file.
        let text = String::from_utf8_lossy(bytes).trim().to_string();
        return (!text.is_empty()).then_some(text);
    }

    None
}

/// True if a file looks like audio (by MIME or extension) — a candidate for
/// transcription rather than text extraction.
pub fn is_audio(filename: &str, mime: &str) -> bool {
    let mime = mime.split(';').next().unwrap_or(mime).trim();
    if mime.starts_with("audio/") {
        return true;
    }
    let ext = filename
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    const AUDIO_EXTS: &[&str] = &[
        "mp3", "m4a", "m4b", "wav", "wave", "ogg", "oga", "opus", "webm", "flac", "aac", "aiff",
        "aif", "wma", "amr",
    ];
    AUDIO_EXTS.contains(&ext.as_str())
}

fn is_text(mime: &str, ext: &str) -> bool {
    let mime = mime.split(';').next().unwrap_or(mime).trim();
    if mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/x-yaml"
                | "application/yaml"
                | "application/csv"
                | "application/toml"
        )
    {
        return true;
    }
    const TEXT_EXTS: &[&str] = &[
        "txt", "md", "markdown", "csv", "tsv", "json", "xml", "yaml", "yml", "toml", "ini", "log",
        "rs", "py", "js", "ts", "tsx", "jsx", "java", "c", "h", "cpp", "hpp", "go", "rb", "php",
        "sh", "bash", "zsh", "sql", "html", "css", "scss", "env", "cfg", "conf",
    ];
    TEXT_EXTS.contains(&ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_audio() {
        assert!(is_audio("note.mp3", ""));
        assert!(is_audio("x", "audio/ogg"));
        assert!(!is_audio("doc.pdf", "application/pdf"));
    }

    #[test]
    fn detects_text() {
        assert!(is_text("text/plain", "txt"));
        assert!(is_text("", "rs"));
        assert!(!is_text("application/octet-stream", "bin"));
    }

    #[test]
    fn extract_text_reads_utf8() {
        assert_eq!(
            extract_text(b"hello world", "a.txt", "text/plain").as_deref(),
            Some("hello world")
        );
        assert!(extract_text(&[0xff, 0xfe], "a.bin", "application/octet-stream").is_none());
    }
}

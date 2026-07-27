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

const CHUNK_CHARS: usize = 1000;
const OVERLAP_CHARS: usize = 150;

/// Split text into overlapping chunks, preferring paragraph/sentence boundaries
/// near the target size so chunks stay coherent.
pub fn chunk(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= CHUNK_CHARS {
        let t = text.trim();
        return if t.is_empty() {
            Vec::new()
        } else {
            vec![t.to_string()]
        };
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let hard_end = (start + CHUNK_CHARS).min(chars.len());
        // Prefer to break on a paragraph, then sentence, then whitespace within
        // the last third of the window.
        let end = if hard_end == chars.len() {
            hard_end
        } else {
            let window_start = start + (CHUNK_CHARS * 2 / 3);
            break_point(&chars, window_start, hard_end).unwrap_or(hard_end)
        };

        let piece: String = chars[start..end].iter().collect();
        let piece = piece.trim().to_string();
        if !piece.is_empty() {
            chunks.push(piece);
        }
        if end >= chars.len() {
            break;
        }
        start = end.saturating_sub(OVERLAP_CHARS);
    }
    chunks
}

/// Best boundary in `[from, to)` — last paragraph break, else sentence end, else
/// whitespace. Returns the index just past the boundary.
fn break_point(chars: &[char], from: usize, to: usize) -> Option<usize> {
    let mut sentence = None;
    let mut whitespace = None;
    for i in (from..to).rev() {
        let c = chars[i];
        if c == '\n' && i + 1 < to && chars[i + 1] == '\n' {
            return Some(i + 1);
        }
        if sentence.is_none() && matches!(c, '.' | '!' | '?') {
            sentence = Some(i + 1);
        }
        if whitespace.is_none() && c.is_whitespace() {
            whitespace = Some(i + 1);
        }
    }
    sentence.or(whitespace)
}

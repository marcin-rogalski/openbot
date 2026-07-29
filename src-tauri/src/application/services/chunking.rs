//! Chunking service: split text into overlapping windows on natural boundaries,
//! ready for embedding. Pure, reusable, no IO — injected into indexing usecases.

const CHUNK_CHARS: usize = 1000;
const OVERLAP_CHARS: usize = 150;

/// Split `text` into ~`CHUNK_CHARS` windows (with `OVERLAP_CHARS` overlap),
/// breaking on paragraph/sentence/whitespace boundaries where possible. Empty or
/// whitespace-only input yields no chunks.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_is_single_chunk() {
        assert_eq!(chunk("short"), vec!["short".to_string()]);
        assert!(chunk("   ").is_empty());
    }

    #[test]
    fn long_text_splits() {
        let text = "sentence. ".repeat(400);
        assert!(chunk(&text).len() > 1);
    }
}

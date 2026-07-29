//! Transcription domain: a timestamped transcript, independent of any model or
//! audio codec. Assembling chunk results (with time offsets) and deriving the
//! plain text are operations on this representation.

/// One timestamped segment: `start` seconds into the recording + its text.
#[derive(Clone, Debug)]
pub struct Segment {
    pub start: f64,
    pub text: String,
}

/// An ordered set of segments with absolute (recording-relative) timestamps.
#[derive(Clone, Debug, Default)]
pub struct Transcript {
    pub segments: Vec<Segment>,
}

impl Transcript {
    /// Append a clip's segments, shifting each timestamp by `offset_secs` (0 for
    /// a single clip; the chunk's start for a chunked recording). Blank segments
    /// are dropped.
    pub fn append(&mut self, segments: Vec<Segment>, offset_secs: f64) {
        for s in segments {
            let text = s.text.trim();
            if text.is_empty() {
                continue;
            }
            self.segments.push(Segment {
                start: (s.start + offset_secs).max(0.0),
                text: text.to_string(),
            });
        }
    }

    /// The transcript as one plain-text string (for summaries / inline context).
    pub fn plain(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_shifts_offsets_and_drops_blanks() {
        let mut t = Transcript::default();
        t.append(
            vec![
                Segment {
                    start: 1.0,
                    text: "a".into(),
                },
                Segment {
                    start: 2.0,
                    text: "  ".into(),
                },
            ],
            300.0,
        );
        assert_eq!(t.segments.len(), 1);
        assert_eq!(t.segments[0].start, 301.0);
        assert_eq!(t.plain(), "a");
    }
}

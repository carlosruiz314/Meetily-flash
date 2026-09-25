use crate::audio::speaker::alignment::TokenWord;

/// whisper.cpp's end-of-turn sentinel. It carries no speech and its timestamps
/// are boundary-degenerate, so it is filtered at every consumption point.
pub fn is_eot_marker(text: &str) -> bool {
    matches!(text.trim(), "_EOT_" | "[_EOT_]")
}

/// Remove embedded EOT markers from whisper segment text.
pub fn strip_eot_markers(text: &str) -> String {
    text.replace("[_EOT_]", " ")
        .replace("_EOT_", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract per-token word timestamps from a Whisper state's segments.
/// Returns a JSON string of TokenWord array, or None if no timestamps available.
pub fn extract_token_timestamps(
    state: &whisper_rs::WhisperState,
    num_segments: i32,
) -> Option<String> {
    let mut words = Vec::new();

    for seg_idx in 0..num_segments {
        let segment = match state.get_segment(seg_idx) {
            Some(s) => s,
            None => continue,
        };

        let n_tokens = segment.n_tokens();
        for tok_idx in 0..n_tokens {
            let token = match segment.get_token(tok_idx) {
                Some(t) => t,
                None => continue,
            };

            let text = match token.to_str_lossy() {
                Ok(t) => t,
                Err(_) => continue,
            };

            let text = text.trim();
            if text.is_empty() || is_eot_marker(text) {
                continue;
            }

            // Skip special/sentinel tokens (whisper-rs assigns them negative token_ids)
            let id = token.token_id();
            if id < 0 {
                continue;
            }

            let data = token.token_data();
            // t0 and t1 are in centiseconds (10ms units)
            let start_ms = data.t0 as i64 * 10;
            let end_ms = data.t1 as i64 * 10;

            // Skip tokens with invalid timestamps
            if start_ms < 0 || end_ms < 0 || end_ms < start_ms {
                continue;
            }

            words.push(TokenWord {
                word: text.to_string(),
                start_ms,
                end_ms,
            });
        }
    }

    if words.is_empty() {
        return None;
    }

    serde_json::to_string(&words).ok()
}

/// Shift token timestamps from whisper-window time into meeting-absolute time.
///
/// whisper.cpp reports token t0/t1 relative to the start of the 30-second
/// window it just processed, but the diarization aligner looks words up
/// against meeting-absolute speaker segments. Every batch chunk (one VAD
/// segment) must therefore add its own `start_timestamp_ms` before the JSON
/// is persisted. Returns None unchanged; an empty word list stays empty.
pub fn offset_token_timestamps(json: &str, offset_ms: i64) -> Option<String> {
    let mut words: Vec<TokenWord> = serde_json::from_str(json).ok()?;
    for w in &mut words {
        w.start_ms += offset_ms;
        w.end_ms += offset_ms;
    }
    serde_json::to_string(&words).ok()
}

/// whisper sometimes emits the same run of pieces twice — the second copy
/// with degenerate walls (all pieces zero-span, pinned to one point, usually
/// the row's end edge). Measured: the cde5c264 source row at 1193.55 s
/// repeated "I am okay to do it as long as they give us an actual plan." with
/// the ghost collapsed to a single point; the render then split the ghost
/// into the NEXT speaker's row, fabricating a verbatim echo. A repeat whose
/// second copy is acoustically weightless is a decode ghost — dropped. Real
/// repeats (people actually saying it twice) carry real walls and stay.
const GHOST_MAX_PIECES: usize = 32;
/// A run whose pieces all sit inside this window is weightless.
const GHOST_MAX_SPAN_MS: i64 = 50;

/// Returns the indices of `items` that survive ghost dedup (order kept).
/// Generic over the item shape: dedup runs at WORD granularity in
/// `valid_token_words` (the ghost's text is part of the row text, so pieces
/// must still merge — only the merged words are droppable).
pub fn dedupe_degenerate_repeats_by<T>(
    items: &[T],
    text: impl Fn(&T) -> &str,
    start: impl Fn(&T) -> i64,
    end: impl Fn(&T) -> i64,
) -> Vec<usize> {
    if items.len() < 2 {
        return (0..items.len()).collect();
    }
    let norm: Vec<String> = items
        .iter()
        .map(|it| {
            text(it).to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
        })
        .collect();
    let weightless = |run: &[T]| {
        let mut lo = i64::MAX;
        let mut hi = i64::MIN;
        for it in run {
            if end(it) - start(it) > 1 {
                return false;
            }
            lo = lo.min(start(it));
            hi = hi.max(end(it));
        }
        hi - lo < GHOST_MAX_SPAN_MS
    };
    let mut out = Vec::with_capacity(items.len());
    let mut i = 0usize;
    while i < items.len() {
        // Longest normalized repeat of the immediately preceding run whose
        // copy is weightless wins; anything else is kept as-is.
        let mut drop_k = 0usize;
        for k in (1..=GHOST_MAX_PIECES.min(i)).rev() {
            if i + k > items.len() {
                continue;
            }
            let prev = norm[i - k..i].concat();
            if prev.is_empty() || prev != norm[i..i + k].concat() {
                continue;
            }
            if weightless(&items[i..i + k]) {
                drop_k = k;
                break;
            }
        }
        if drop_k > 0 {
            i += drop_k;
            continue;
        }
        out.push(i);
        i += 1;
    }
    out
}

/// TokenWord convenience wrapper (piece-level callers; tests).
pub fn dedupe_degenerate_repeats(pieces: &[TokenWord]) -> Vec<usize> {
    dedupe_degenerate_repeats_by(
        pieces,
        |p| &p.word,
        |p| p.start_ms,
        |p| p.end_ms,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::speaker::alignment::TokenWord;

    #[test]
    fn offset_shifts_every_word_by_the_chunk_start() {
        let json = r#"[{"word":"Hello","start_ms":0,"end_ms":500},
                       {"word":"world","start_ms":500,"end_ms":1500}]"#;
        let out = offset_token_timestamps(json, 41_000).expect("offset ok");
        let parsed: Vec<TokenWord> = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed[0].start_ms, 41_000);
        assert_eq!(parsed[0].end_ms, 41_500);
        assert_eq!(parsed[1].start_ms, 41_500);
        assert_eq!(parsed[1].end_ms, 42_500);
    }

    #[test]
    fn offset_preserves_none_and_empty() {
        assert!(offset_token_timestamps("null", 100).is_none());
        assert_eq!(offset_token_timestamps("[]", 100).as_deref(), Some("[]"));
    }

    #[test]
    fn invalid_json_returns_none() {
        assert!(offset_token_timestamps("not json", 5).is_none());
    }

    #[test]
    fn detects_eot_marker_forms() {
        assert!(is_eot_marker("_EOT_"));
        assert!(is_eot_marker("[_EOT_]"));
        assert!(is_eot_marker(" [_EOT_] "));
        assert!(!is_eot_marker("hello"));
        assert!(!is_eot_marker(""));
        assert!(!is_eot_marker("[_EOT_] extra"));
    }

    fn tw(w: &str, s: i64, e: i64) -> TokenWord {
        TokenWord { word: w.to_string(), start_ms: s, end_ms: e }
    }

    #[test]
    fn drops_zero_wall_ghost_repeat() {
        // Real shape (cde5c264 1193.55 row): "I am okay … plan." with real
        // walls, then the same run with every piece zero-span at the row end.
        let pieces = vec![
            tw("I", 1_200_400, 1_200_470),
            tw("am", 1_200_710, 1_200_860),
            tw("okay", 1_201_210, 1_202_000),
            tw("to", 1_202_300, 1_202_500),
            tw("do", 1_202_600, 1_202_800),
            tw("it", 1_202_900, 1_203_000),
            tw("as", 1_203_100, 1_203_200),
            tw("long", 1_203_210, 1_203_400),
            tw("as", 1_203_410, 1_203_500),
            tw("they", 1_203_600, 1_203_800),
            tw("give", 1_203_900, 1_204_100),
            tw("us", 1_204_200, 1_204_300),
            tw("an", 1_204_400, 1_204_450),
            tw("actual", 1_204_500, 1_204_800),
            tw("plan", 1_204_900, 1_205_100),
            tw("I", 1_215_600, 1_215_600),
            tw("am", 1_215_600, 1_215_600),
            tw("okay", 1_215_600, 1_215_600),
            tw("to", 1_215_600, 1_215_600),
            tw("do", 1_215_600, 1_215_600),
            tw("it", 1_215_600, 1_215_600),
            tw("as", 1_215_600, 1_215_600),
            tw("long", 1_215_600, 1_215_600),
            tw("as", 1_215_600, 1_215_600),
            tw("they", 1_215_600, 1_215_600),
            tw("give", 1_215_600, 1_215_600),
            tw("us", 1_215_600, 1_215_600),
            tw("an", 1_215_600, 1_215_600),
            tw("actual", 1_215_600, 1_215_600),
            tw("plan", 1_215_600, 1_215_600),
        ];
        let kept: Vec<&TokenWord> =
            dedupe_degenerate_repeats(&pieces).into_iter().map(|i| &pieces[i]).collect();
        assert_eq!(kept.len(), 15, "ghost run dropped, real copy intact");
        assert_eq!(kept.last().unwrap().word, "plan");
        assert_eq!(kept[0].start_ms, 1_200_400);
    }

    #[test]
    fn keeps_real_repeats() {
        // People actually repeating: real walls on both copies.
        let pieces = vec![
            tw("No", 100, 200),
            tw("no", 300, 400),
            tw("no", 500, 600),
        ];
        assert_eq!(dedupe_degenerate_repeats(&pieces).len(), 3);
        let echo = vec![tw("hi", 1_000, 1_100), tw("hi", 2_000, 2_100)];
        assert_eq!(dedupe_degenerate_repeats(&echo).len(), 2);
    }

    #[test]
    fn both_weightless_drops_second_copy_only() {
        // Whole region degenerate: the first copy may still be resolvable by
        // the wall merge; only the repeat is dropped.
        let pieces = vec![tw("word", 5_000, 5_000), tw("word", 5_000, 5_000)];
        let kept: Vec<usize> = dedupe_degenerate_repeats(&pieces);
        assert_eq!(kept, vec![0]);
    }

    #[test]
    fn near_identical_but_divergent_text_is_kept() {
        // "plan" vs "a" — normalization differs, nothing drops.
        let pieces = vec![tw("plan", 1_000, 1_200), tw("a", 1_215_600, 1_215_600)];
        assert_eq!(dedupe_degenerate_repeats(&pieces).len(), 2);
    }

    #[test]
    fn strip_eot_markers_removes_embedded_and_standalone() {
        assert_eq!(strip_eot_markers("hello [_EOT_] world"), "hello world");
        assert_eq!(strip_eot_markers("[_EOT_]"), "");
        assert_eq!(strip_eot_markers("start _EOT_ end"), "start end");
        assert_eq!(strip_eot_markers("no markers here"), "no markers here");
    }

    #[test]
    fn serialize_token_words() {
        let words = vec![
            TokenWord {
                word: "Hello".to_string(),
                start_ms: 0,
                end_ms: 500,
            },
            TokenWord {
                word: "world".to_string(),
                start_ms: 500,
                end_ms: 1000,
            },
        ];
        let json = serde_json::to_string(&words).unwrap();
        assert!(json.contains("Hello"));
        assert!(json.contains("world"));

        let parsed: Vec<TokenWord> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].word, "Hello");
        assert_eq!(parsed[0].start_ms, 0);
        assert_eq!(parsed[0].end_ms, 500);
    }

    #[test]
    fn serialize_non_ascii_tokens() {
        let words = vec![
            TokenWord {
                word: "café".to_string(),
                start_ms: 0,
                end_ms: 500,
            },
            TokenWord {
                word: "niño".to_string(),
                start_ms: 500,
                end_ms: 1000,
            },
            TokenWord {
                word: "über".to_string(),
                start_ms: 1000,
                end_ms: 1500,
            },
        ];
        let json = serde_json::to_string(&words).unwrap();
        let parsed: Vec<TokenWord> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0].word, "café");
        assert_eq!(parsed[1].word, "niño");
        assert_eq!(parsed[2].word, "über");
    }

    #[test]
    fn serialize_large_segment() {
        let words: Vec<TokenWord> = (0..600)
            .map(|i| TokenWord {
                word: format!("word{}", i),
                start_ms: i * 100,
                end_ms: (i + 1) * 100,
            })
            .collect();
        let json = serde_json::to_string(&words).unwrap();
        let parsed: Vec<TokenWord> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 600);
    }

    #[test]
    fn deserialize_malformed_json_falls_back() {
        let bad_json = r#"not valid json"#;
        let result: Result<Vec<TokenWord>, _> = serde_json::from_str(bad_json);
        assert!(result.is_err());

        // Missing fields
        let missing = r#"[{"word": "test"}]"#;
        let result: Result<Vec<TokenWord>, _> = serde_json::from_str(missing);
        assert!(result.is_err());

        // Negative timestamps — our code validates in alignment
        let negative = r#"[{"word": "test", "start_ms": -1, "end_ms": 100}]"#;
        let parsed: Vec<TokenWord> = serde_json::from_str(negative).unwrap();
        assert_eq!(parsed[0].start_ms, -1); // deserialized OK, validated at alignment
    }
}

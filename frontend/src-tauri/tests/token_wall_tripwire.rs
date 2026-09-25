//! Tripwire for the silent proportional-wall collapse. The 2026-09 defect: a
//! token validator rejected EVERY whisper row (BPE piece count vs whitespace
//! word count — the counts never match), so all 214 tokened rows silently fell
//! back to proportional walls and the meeting rendered on ±seconds drift.
//! This test replays the validator over the pinned fixture rows in seconds —
//! no audio, no models, no DB — and fails loudly when tokened rows lose their
//! real word walls.

use app_lib::audio::speaker::alignment::{valid_token_words, TranscriptInput};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    rows: Vec<FixtureRow>,
}

#[derive(Deserialize)]
struct FixtureRow {
    id: String,
    text: String,
    start_ms: i64,
    end_ms: i64,
    token_timestamps: Option<String>,
}

/// Floor over the pinned cde5c264 snapshot (measured 2026-09-24: 176/229 —
/// the 53 fallbacks are genuinely divergent piece streams, not count-mismatch
/// rejections). Only change it alongside a deliberate fixture re-pin; a drop
/// means the validator is rejecting rows whisper timestamped correctly.
const MIN_REAL_WALL_ROWS: usize = 176;

#[test]
fn every_tokened_fixture_row_keeps_its_real_word_walls() {
    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/cde5c264_transcripts.json"
    ))
    .expect("read cde5c264_transcripts.json");
    let fixture: Fixture = serde_json::from_str(&raw).expect("parse fixture");
    assert!(
        !fixture.rows.is_empty(),
        "fixture has 0 rows — never a vacuous green"
    );

    let mut tokened = 0usize;
    let mut real = 0usize;
    let mut fallback: Vec<&str> = Vec::new();
    for (i, r) in fixture.rows.iter().enumerate() {
        let input = TranscriptInput {
            id: r.id.clone(),
            text: r.text.clone(),
            audio_start_ms: r.start_ms,
            audio_end_ms: r.end_ms,
            token_words: r
                .token_timestamps
                .as_deref()
                .and_then(|json| serde_json::from_str(json).ok()),
        };
        if !input.token_words.as_ref().is_some_and(|w| !w.is_empty()) {
            continue; // genuinely tokenless rows fall back honestly
        }
        tokened += 1;
        if valid_token_words(&input, i).is_some() {
            real += 1;
        } else {
            fallback.push(&r.id);
        }
    }
    eprintln!(
        "TRIPWIRE: {real}/{tokened} tokened rows on real word walls ({} fallback)",
        fallback.len()
    );
    for id in fallback.iter().take(20) {
        eprintln!("  fallback row: {id}");
    }
    assert!(
        tokened > 0,
        "fixture has 0 tokened rows — re-pin from transcript_sources"
    );
    assert!(
        real >= MIN_REAL_WALL_ROWS,
        "token-wall collapse: only {real}/{tokened} tokened rows kept real walls \
         (floor {MIN_REAL_WALL_ROWS}); fallback ids: {:?}. The validator is \
         rejecting rows whisper timestamped correctly — fix valid_token_words, \
         don't loosen this pin.",
        fallback.iter().take(10).collect::<Vec<_>>()
    );
}

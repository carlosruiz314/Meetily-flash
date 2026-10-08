//! Census-replay harness (diarization-render-fidelity section 4, design D6).
//!
//! Fast-fail tier of the verification ladder (unit → warm replay → span
//! probe → cold gate): synthetic, token-only fixtures drive the REAL pure
//! splice functions in milliseconds — `decode_span_synthesis` (with a fake
//! decoder), `decode_stream_utterances`, `apply_overlap_synthesis`, and
//! `apply_loop_repairs`. No meeting audio, no models, no GPU.
//!
//! The fake-decoder contract is keyed by (span, stream) via per-call
//! closure scoping: the harness invokes the synthesis entry with ONE input
//! per call and a closure bound to that input's expected samples plus a
//! call counter — never a re-implemented decode loop. The contract pins:
//! (a) decode-once per stream, (b) decode inputs byte-identical to the
//! pinned stream samples (a re-slice fails here, not at a ~70 min gate).
//!
//! Fixture tiers (see tests/fixtures/census_replay/README.md): the
//! repo-committed fixtures are PURELY SYNTHETIC; meeting-shaped fixtures
//! load from `MEETILY_LOCAL_EVIDENCE_DIR/census_replay/` when present and
//! skip with a loud note when absent (the live gate is their parity pin).

#![cfg(test)]

use proptest::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use app_lib::audio::speaker::alignment::{AlignedSegment, SpeakerSource};
use app_lib::audio::speaker::commands::{apply_overlap_synthesis, decode_span_synthesis};
use app_lib::audio::speaker::run_assembly::{
    apply_loop_repairs, decode_stream_utterances, SpanSynthesis, StreamEvidence, StreamUtterance,
};
use app_lib::audio::speaker::run_engine::{SpanSynthesisInput, SAMPLE_RATE};

// ── Fixture schema ──

#[derive(serde::Deserialize)]
struct ReplayFixture {
    id: String,
    #[serde(default)]
    note: String,
    rows: Vec<RowSpec>,
    span_s: (f64, f64),
    #[serde(default)]
    covered_atoms_ms: Vec<(i64, i64)>,
    identity: [Option<(u32, f32)>; 2],
    rms_ratio: [f32; 2],
    streams: [StreamScript; 2],
    #[serde(default)]
    manual_spans_ms: Vec<(i64, i64)>,
    #[serde(default)]
    label_map: HashMap<String, String>,
    expected: Expectation,
}

#[derive(serde::Deserialize)]
struct StreamScript {
    /// Tone windows (ms, stream-relative): sine inside, silence outside —
    /// the synthetic audio the REAL energy splitter runs on.
    tone_windows_ms: Vec<(i64, i64)>,
    /// The scripted WHOLE-stream decode text (what the fake decoder
    /// returns for the full stream samples).
    decode_text: Option<String>,
}

#[derive(serde::Deserialize)]
struct RowSpec {
    id: String,
    text: String,
    start_ms: i64,
    end_ms: i64,
    speaker: String,
    #[serde(default)]
    synth_atom: bool,
}

#[derive(serde::Deserialize)]
struct Expectation {
    /// "degrade" = byte-identical mixture rows; "replace" = pinned shapes.
    mode: String,
    #[serde(default)]
    rows: Vec<RowSpec>,
    #[serde(default)]
    wall_tol_ms: Option<i64>,
    /// The veto reason `synthesis_veto_reason` must report for this
    /// fixture — pins WHICH gate fired, so a vetting reorder fails the
    /// replay instead of silently keeping the shape green. `null`/absent
    /// = the candidate must pass.
    #[serde(default)]
    veto: Option<String>,
}

impl RowSpec {
    fn to_segment(&self) -> AlignedSegment {
        AlignedSegment {
            original_id: self.id.clone(),
            text: self.text.clone(),
            audio_start_ms: self.start_ms,
            audio_end_ms: self.end_ms,
            speaker: self.speaker.clone(),
            speaker_source: SpeakerSource::Auto,
            synth_atom: self.synth_atom,
            synth_parent: None,
        }
    }
}

// ── Synthetic audio ──

/// Sine tone inside the windows, silence elsewhere — enough contrast for
/// the peak-anchored energy splitter to find the utterance spans.
fn synth_samples(windows: &[(i64, i64)]) -> Vec<f32> {
    let end_ms = windows.iter().map(|w| w.1).max().unwrap_or(0).max(1) as usize;
    let len = end_ms * SAMPLE_RATE as usize / 1000;
    let mut v = vec![0.0f32; len];
    for &(a, b) in windows {
        let a_s = a as usize * SAMPLE_RATE as usize / 1000;
        let b_s = (b as usize * SAMPLE_RATE as usize / 1000).min(len);
        for (i, s) in v.iter_mut().enumerate().skip(a_s).take(b_s.saturating_sub(a_s)) {
            *s = 0.1 * (2.0 * std::f64::consts::PI * 440.0 * i as f64 / SAMPLE_RATE as f64).sin()
                as f32;
        }
    }
    v
}

// ── Replay ──

/// Drives the REAL synthesis entry with ONE input and a closure bound to
/// that input's expected contract + call counters (D6) — never a
/// re-implemented decode loop.
fn replay(fx: &ReplayFixture) -> Vec<AlignedSegment> {
    let streams = [
        synth_samples(&fx.streams[0].tone_windows_ms),
        synth_samples(&fx.streams[1].tone_windows_ms),
    ];
    let input = SpanSynthesisInput {
        span: fx.span_s,
        identity: fx.identity,
        rms_ratio: fx.rms_ratio,
        streams: streams.clone(),
        covered_atoms: fx.covered_atoms_ms.clone(),
    };
    let texts = [fx.streams[0].decode_text.clone(), fx.streams[1].decode_text.clone()];
    // An undecided identity skips the input BEFORE any decode — the
    // expected call count is part of the pinned contract.
    let expected_calls = if fx.identity.iter().all(Option::is_some) { 2 } else { 0 };
    let outer_calls = RefCell::new(0usize);
    let fake = |samples: &[f32]| -> Vec<StreamUtterance> {
        let idx = {
            let mut c = outer_calls.borrow_mut();
            let i = *c;
            *c += 1;
            i
        };
        // Call order binds stream index: a production swap feeds stream-1
        // samples where stream-0 samples are expected and fails the
        // byte-identity assert below.
        assert!(
            idx < 2,
            "decode_span_synthesis called the decoder more than once per stream pair"
        );
        let expected = streams[idx].clone();
        let scripted = texts[idx].clone();
        let decode_calls = RefCell::new(0usize);
        let inner = move |s: &[f32]| -> Option<String> {
            let n = {
                let mut c = decode_calls.borrow_mut();
                *c += 1;
                *c
            };
            assert_eq!(n, 1, "decode-once contract: the stream decoder ran {n} times");
            assert_eq!(
                s,
                expected.as_slice(),
                "decode inputs changed: the synthesis path re-sliced the stream"
            );
            scripted.clone()
        };
        decode_stream_utterances(samples, &inner)
    };

    let synthesis = decode_span_synthesis(vec![input], &fake);
    assert_eq!(
        *outer_calls.borrow(),
        expected_calls,
        "decode call count deviates from the pinned contract"
    );
    let expected_candidates = if expected_calls == 2 { 1 } else { 0 };
    assert_eq!(
        synthesis.len(),
        expected_candidates,
        "candidate count deviates from the pinned contract"
    );
    let rows: Vec<AlignedSegment> = fx.rows.iter().map(|r| r.to_segment()).collect();
    let label_map: HashMap<u32, String> = fx
        .label_map
        .iter()
        .filter_map(|(k, v)| k.parse::<u32>().ok().map(|c| (c, v.clone())))
        .collect();
    // Reason pin (build-panel finding: shape-only asserts are blind to
    // gate reorderings): the shared veto fn names WHICH gate fired.
    let veto = app_lib::audio::speaker::run_assembly::synthesis_veto_reason(
        synthesis.first(),
        &|c: u32| label_map.get(&c).cloned(),
    );
    match (&fx.expected.veto, veto) {
        (None, None) => {}
        (Some(want), Some(got)) => assert_eq!(
            got, want,
            "[{}] the wrong gate fired",
            fx.id
        ),
        (Some(want), None) => panic!("[{}] expected veto {want:?} but the splice passed", fx.id),
        (None, Some(got)) => panic!("[{}] unexpected veto: {got}", fx.id),
    }
    let (out, _synthesized) =
        apply_overlap_synthesis(rows, &synthesis, &fx.manual_spans_ms, &label_map);
    out
}

fn assert_expected(fx: &ReplayFixture, out: &[AlignedSegment]) {
    match fx.expected.mode.as_str() {
        "degrade" => {
            assert_eq!(
                out.len(),
                fx.rows.len(),
                "[{}] degrade must be byte-identical: row count",
                fx.id
            );
            for (o, r) in out.iter().zip(&fx.rows) {
                assert_eq!(o.original_id, r.id, "[{}] provenance", fx.id);
                assert_eq!(o.text, r.text, "[{}] text", fx.id);
                assert_eq!(
                    (o.audio_start_ms, o.audio_end_ms),
                    (r.start_ms, r.end_ms),
                    "[{}] walls",
                    fx.id
                );
                assert_eq!(o.speaker, r.speaker, "[{}] badge", fx.id);
                assert!(!o.synth_atom, "[{}] no synth atom survives a degrade", fx.id);
                assert!(o.synth_parent.is_none(), "[{}] no parent link on degrade", fx.id);
            }
        }
        "replace" => {
            let exp = &fx.expected.rows;
            assert!(
                !exp.is_empty(),
                "[{}] replace mode pins the expected rows",
                fx.id
            );
            assert_eq!(out.len(), exp.len(), "[{}] row count: {out:?}", fx.id);
            let tol = fx.expected.wall_tol_ms.unwrap_or(0);
            for (o, e) in out.iter().zip(exp) {
                assert_eq!(o.text, e.text, "[{}] text", fx.id);
                assert_eq!(o.speaker, e.speaker, "[{}] badge for {:?}", fx.id, e.text);
                assert_eq!(o.synth_atom, e.synth_atom, "[{}] atom flag for {:?}", fx.id, e.text);
                assert_eq!(o.original_id, e.id, "[{}] provenance for {:?}", fx.id, e.text);
                assert!(
                    (o.audio_start_ms - e.start_ms).abs() <= tol
                        && (o.audio_end_ms - e.end_ms).abs() <= tol,
                    "[{}] walls for {:?}: got [{},{}], want [{},{}]±{}",
                    fx.id,
                    e.text,
                    o.audio_start_ms,
                    o.audio_end_ms,
                    e.start_ms,
                    e.end_ms,
                    tol
                );
            }
        }
        other => panic!("[{}] unknown expectation mode {other}", fx.id),
    }
}

// ── Loaders (two-tier split) ──

fn parse_fixtures(dir: &PathBuf, tier: &str, out: &mut Vec<ReplayFixture>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("[{tier}] reading {}: {e}", p.display()));
        let fx: ReplayFixture =
            serde_json::from_str(&raw).unwrap_or_else(|e| panic!("[{tier}] parsing {}: {e}", p.display()));
        out.push(fx);
    }
    let _ = tier;
}

fn load_fixtures() -> Vec<ReplayFixture> {
    let mut out = Vec::new();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/census_replay");
    parse_fixtures(&repo, "synthetic", &mut out);
    assert!(!out.is_empty(), "no synthetic census-replay fixtures found");

    // Meeting-shaped tier: lives in the protected evidence home (the PII
    // bright line covers meeting-derived synthetic data too). Skipped with
    // a loud note when absent — the live gate is its parity pin.
    match std::env::var("MEETILY_LOCAL_EVIDENCE_DIR") {
        Err(_) => eprintln!(
            "HARNESS: MEETILY_LOCAL_EVIDENCE_DIR unset — meeting-shaped fixtures skipped (the live ear gate is their parity pin)"
        ),
        Ok(home) => {
            let dir = PathBuf::from(&home).join("census_replay");
            if dir.is_dir() {
                let before = out.len();
                parse_fixtures(&dir, "meeting-shaped", &mut out);
                eprintln!(
                    "HARNESS: loaded {} meeting-shaped fixture(s) from {}",
                    out.len() - before,
                    dir.display()
                );
            } else {
                eprintln!(
                    "HARNESS: MEETILY_LOCAL_EVIDENCE_DIR set but {} is missing — meeting-shaped tier skipped",
                    dir.display()
                );
            }
        }
    }
    out
}

#[test]
fn synthetic_fixtures_replay_to_the_pinned_shapes() {
    let fixtures = load_fixtures();
    for fx in &fixtures {
        let out = replay(fx);
        assert_expected(fx, &out);
    }
}

// ── Property tests (D6) ──

fn rows_eq(a: &[AlignedSegment], b: &[AlignedSegment]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.original_id == y.original_id
                && x.text == y.text
                && x.audio_start_ms == y.audio_start_ms
                && x.audio_end_ms == y.audio_end_ms
                && x.speaker == y.speaker
                && x.synth_atom == y.synth_atom
                && x.synth_parent == y.synth_parent
        })
}

fn word_list(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_string).collect()
}

/// An ordered subsequence (not necessarily contiguous): the split pieces
/// must never invent, reorder, or duplicate absorbed words.
fn is_subsequence(needle: &[String], haystack: &[String]) -> bool {
    let mut it = haystack.iter();
    needle.iter().all(|n| it.any(|h| h == n))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn straddling_split_never_invents_or_reorders_piece_words(
        words in prop::collection::vec("[a-z]{2,8}", 3..=12),
        row_start in 0i64..5_000,
        row_dur in 8_000i64..30_000,
        cut0 in 10i64..=70,
        gap in 13i64..=20,
    ) {
        let cut1 = cut0 + gap;
        prop_assume!(cut0 < cut1 && cut1 <= 90);
        let row_end = row_start + row_dur;
        let span0 = row_start + row_dur * cut0 / 100;
        let span1 = row_start + row_dur * cut1 / 100;
        prop_assume!(span1 > span0 && (span1 - span0) >= 1_500);
        let text = words.join(" ");
        let rows = vec![AlignedSegment {
            original_id: "src1".into(),
            text: text.clone(),
            audio_start_ms: row_start,
            audio_end_ms: row_end,
            speaker: "Speaker 0".into(),
            speaker_source: SpeakerSource::Auto,
            synth_atom: false,
            synth_parent: None,
        }];
        let synthesis = SpanSynthesis {
            span: (span0 as f64 / 1000.0, span1 as f64 / 1000.0),
            streams: [
                StreamEvidence { cluster: 1, margin: 0.3, rms_ratio: 1.0, text: Some("one two".into()), utterances: Vec::new() },
                StreamEvidence { cluster: 3, margin: 0.2, rms_ratio: 1.0, text: Some("three four".into()), utterances: Vec::new() },
            ],
            covered_atoms: vec![(span0, span1)],
        };
        let cluster_speaker = |c: u32| Some(format!("spk{c}"));
        let out = app_lib::audio::speaker::run_assembly::synthesize_overlap_rows(
            &rows,
            &synthesis,
            &[],
            &cluster_speaker,
        )
        .expect("straddling span with passing evidence synthesizes");

        // Wall monotonicity across the whole replacement.
        for w in out.windows(2) {
            prop_assert!(w[0].audio_start_ms <= w[1].audio_start_ms, "rows sorted by start");
        }
        // Span clamping: atom rows inside the span walls; pieces outside.
        for r in &out {
            prop_assert!(r.audio_start_ms >= row_start && r.audio_end_ms <= row_end, "walls within the input bounds");
            if r.synth_atom {
                // ±1ms: the splice reconstructs span_ms from f64 seconds
                // (truncating), so the integer-path walls can sit 1ms off.
                prop_assert!(
                    r.audio_start_ms >= span0 - 1 && r.audio_end_ms <= span1 + 1,
                    "atom walls clamped to the span (±1ms f64 round-trip)"
                );
            }
        }
        // Provenance containment: every output id comes from the input.
        for r in &out {
            prop_assert_eq!(&r.original_id, "src1");
        }
        // Piece-word containment: pieces are ordered subsequences of the
        // absorbed words — the split never invents or reorders.
        let original = word_list(&text);
        for r in out.iter().filter(|r| !r.synth_atom) {
            let piece_words = word_list(&r.text);
            prop_assert!(
                is_subsequence(&piece_words, &original),
                "piece {:?} must be an ordered subsequence of the absorbed row",
                r.text
            );
        }
        // Combined-piece containment: head+tail concatenated in output
        // order is ONE ordered subsequence — per-piece checks alone are
        // blind to cross-piece duplication (the 2026-09-30 live-persist
        // bug class put the FULL text in both pieces). Words proportionally
        // inside the span are dropped from the pieces BY DESIGN (owned by
        // the stream rows; pinned by the lib split tests), so the
        // containment direction is all this property can assert.
        let mut combined: Vec<String> = Vec::new();
        for r in out.iter().filter(|r| !r.synth_atom) {
            combined.extend(word_list(&r.text));
        }
        prop_assert!(
            is_subsequence(&combined, &original),
            "pieces must JOINTLY be an ordered subsequence — cross-piece duplication or reorder"
        );
    }

    #[test]
    fn repair_application_is_an_absorption_fixpoint(
        row_dur in 2_000i64..10_000,
        window_pad in 100i64..400,
    ) {
        let rows = vec![
            AlignedSegment {
                original_id: "src1".into(),
                text: "like that like that".into(),
                audio_start_ms: 1_000,
                audio_end_ms: 1_000 + row_dur,
                speaker: "Speaker 0".into(),
                speaker_source: SpeakerSource::Auto,
                synth_atom: false,
                synth_parent: None,
            },
            AlignedSegment {
                original_id: "src2".into(),
                text: "unrelated row survives".into(),
                audio_start_ms: 1_000 + row_dur + 500,
                audio_end_ms: 1_000 + row_dur + 500 + row_dur,
                speaker: "Speaker 1".into(),
                speaker_source: SpeakerSource::Auto,
                synth_atom: false,
                synth_parent: None,
            },
        ];
        let w0 = 1_000i64.saturating_sub(window_pad);
        let w1 = 1_000 + row_dur + window_pad;
        let repair = SpanSynthesis {
            span: (w0 as f64 / 1000.0, w1 as f64 / 1000.0),
            streams: [
                StreamEvidence { cluster: 1, margin: 0.3, rms_ratio: 1.0, text: Some("clean words".into()), utterances: Vec::new() },
                StreamEvidence { cluster: 3, margin: 0.2, rms_ratio: 1.0, text: Some("other voice".into()), utterances: Vec::new() },
            ],
            covered_atoms: vec![(1_000, 1_000 + row_dur)],
        };
        let repair2 = SpanSynthesis {
            span: repair.span,
            streams: [
                StreamEvidence {
                    cluster: 1,
                    margin: 0.3,
                    rms_ratio: 1.0,
                    text: Some("clean words".into()),
                    utterances: Vec::new(),
                },
                StreamEvidence {
                    cluster: 3,
                    margin: 0.2,
                    rms_ratio: 1.0,
                    text: Some("other voice".into()),
                    utterances: Vec::new(),
                },
            ],
            covered_atoms: repair.covered_atoms.clone(),
        };
        let cluster_speaker = |c: u32| Some(format!("spk{c}"));
        let (out1, applied1, _) =
            apply_loop_repairs(rows.clone(), &[repair], &cluster_speaker);
        prop_assert_eq!(applied1, 1, "the window repairs once");
        let (out2, applied2, _) = apply_loop_repairs(out1.clone(), &[repair2], &cluster_speaker);
        prop_assert_eq!(applied2, 0, "re-applying is rejected: the region is already per-voice");
        prop_assert!(rows_eq(&out1, &out2), "the repaired render is a fixpoint");
        prop_assert!(out1.iter().any(|r| r.original_id == "src2"), "the untouched row survives");
    }
}


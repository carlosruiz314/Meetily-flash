//! Ear-truth fixture gate (change `hybrid-diarization-engine`, tasks 1.2/1.4).
//!
//! MEETIFY_LIVE_DIAG=1 cargo test --test ear_truth_gate -- --ignored --nocapture
//! (run via `openspec/changes/hybrid-diarization-engine/run_gate.bat`, which
//! records every run under that change's `gate-runs/`).
//!
//! Runs the CURRENT production attribution path on the real meeting audio and
//! asserts every entry in `tests/fixtures/ear_truth_cde5c264.json`. Expected
//! RED on the legacy pipeline — that is the detection-power proof. Goes GREEN
//! only via the run-assembly engine (task 4.1) or per-entry KNOWN-LIMITATION
//! listings in the fixture JSON with explicit user sign-off.
//!
//! Semantics: `single_voice` = every segment overlapping the span by >0.25s
//! carries the same label (silence-delimited same-speaker boundaries are not
//! violations — the ear attests voices, not turn units). `voice_change_at` =
//! exactly one label change inside the span, and at least one within the
//! pinned tolerance of `change_at_s`.

use app_lib::audio::speaker::diarization::DiarizationPort;
use app_lib::audio::speaker::sherpa_adapter::OrtDiarizationAdapter;
use serde::Deserialize;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

const MODELS_DIR: &str = ".meetily-models";
const AUDIO: &str = "Music/meetily-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";
const THRESHOLD: f32 = 0.65;
const OVERLAP_EPS: f64 = 0.25;

#[derive(Deserialize)]
struct Fixture {
    #[serde(default)]
    meeting: String,
    entries: Vec<Entry>,
    #[serde(default)]
    known_limitations: Vec<String>,
}

#[derive(Deserialize)]
struct Entry {
    id: String,
    kind: String,
    start_s: f64,
    end_s: f64,
    #[serde(default)]
    change_at_s: Option<f64>,
    #[serde(default)]
    tolerance_s: Option<f64>,
    #[serde(default)]
    hold_out: bool,
}

/// A derived attribution segment (time-ordered, non-overlapping).
struct Seg {
    start: f64,
    end: f64,
    label: u32,
}

enum Verdict {
    Pass,
    Fail(String),
}

fn check_single_voice(segs: &[Seg], e: &Entry) -> Verdict {
    let mut overlapping: Vec<(f64, f64, u32)> = Vec::new();
    for s in segs {
        let ov = s.end.min(e.end_s) - s.start.max(e.start_s);
        if ov > OVERLAP_EPS {
            overlapping.push((s.start, s.end, s.label));
        }
    }
    if overlapping.is_empty() {
        return Verdict::Fail("no segments overlap the span".into());
    }
    let first = overlapping[0].2;
    let mismatched: Vec<(f64, f64, u32)> = overlapping
        .iter()
        .copied()
        .filter(|(_, _, l)| *l != first)
        .collect();
    if mismatched.is_empty() {
        Verdict::Pass
    } else {
        Verdict::Fail(format!(
            "label flip inside single-voice span: expected {first} everywhere, found {:?}",
            mismatched
        ))
    }
}

fn check_voice_change(segs: &[Seg], e: &Entry) -> Verdict {
    let Some(change_at) = e.change_at_s else {
        return Verdict::Fail("voice_change_at entry missing change_at_s".into());
    };
    let tol = e.tolerance_s.unwrap_or(0.5);
    // Label-change transition points, in time order.
    let mut changes: Vec<(f64, u32, u32)> = Vec::new();
    for w in segs.windows(2) {
        if w[0].label != w[1].label {
            changes.push((w[1].start, w[0].label, w[1].label));
        }
    }
    let in_span: Vec<&(f64, u32, u32)> = changes
        .iter()
        .filter(|(t, _, _)| *t >= e.start_s && *t <= e.end_s)
        .collect();
    if in_span.len() != 1 {
        return Verdict::Fail(format!(
            "expected exactly 1 label change in span, found {}: {:?}",
            in_span.len(),
            in_span
        ));
    }
    let (t, from, to) = in_span[0];
    if (t - change_at).abs() <= tol {
        Verdict::Pass
    } else {
        Verdict::Fail(format!(
            "change at {t:.2}s ({from}→{to}) is outside tolerance of pinned {change_at}±{tol}"
        ))
    }
}

#[tokio::test]
#[ignore = "live GPU gate: MEETIFY_LIVE_DIAG=1 cargo test --test ear_truth_gate -- --ignored --nocapture"]
async fn ear_truth_gate_cde5c264() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let home = std::env::var("USERPROFILE").unwrap();
    let fixture: Fixture = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ear_truth_cde5c264.json"
        ))
        .expect("read fixture JSON"),
    )
    .expect("parse fixture JSON");

    let audio_path = format!("{home}/{AUDIO}");
    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();
    eprintln!("GATE: decoded {:.1}s", decoded.duration_seconds);

    let speech = app_lib::audio::vad::get_speech_chunks(&samples, 2000).expect("vad");
    let regions: Vec<(f64, f64)> = speech
        .iter()
        .map(|s| (s.start_timestamp_ms / 1000.0, s.end_timestamp_ms / 1000.0))
        .collect();

    let models_dir = format!("{home}/{MODELS_DIR}");
    let emb_path = format!(
        "{}/{}",
        models_dir,
        app_lib::audio::speaker::model_download::embedding_filename()
    );
    let seg_path = format!("{models_dir}/pyannote-segmentation.onnx");
    let fp = (THRESHOLD * 65536.0) as u32;
    let adapter = OrtDiarizationAdapter::with_shared_threshold(
        &emb_path,
        &seg_path,
        Arc::new(AtomicU32::new(fp)),
    )
    .expect("adapter");
    let out = adapter
        .process(&samples, 16_000, &regions)
        .expect("diarization process");
    let final_segments = adapter
        .refine_pass2(&samples, 16_000, &out.centroids.clone())
        .expect("pass2");
    eprintln!(
        "GATE: coarse segments={}, final segments={}",
        out.segments.len(),
        final_segments.len()
    );

    let mut segs: Vec<Seg> = final_segments
        .iter()
        .map(|s| Seg {
            start: s.start_seconds,
            end: s.end_seconds,
            label: s.speaker_id,
        })
        .collect();
    segs.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());

    let mut failed: Vec<String> = Vec::new();
    let mut passed = 0usize;
    let mut limited = 0usize;
    for e in &fixture.entries {
        let verdict = match e.kind.as_str() {
            "single_voice" => check_single_voice(&segs, e),
            "voice_change_at" => check_voice_change(&segs, e),
            other => Verdict::Fail(format!("unknown kind {other}")),
        };
        let tag = if e.hold_out { " [hold-out]" } else { "" };
        match verdict {
            Verdict::Pass => {
                passed += 1;
                eprintln!("PASS{} {}", tag, e.id);
            }
            Verdict::Fail(reason) => {
                if fixture.known_limitations.iter().any(|k| k == &e.id) {
                    limited += 1;
                    eprintln!("KNOWN-LIMITATION{} {} — {}", tag, e.id, reason);
                } else {
                    failed.push(e.id.clone());
                    eprintln!("FAIL{} {} — {}", tag, e.id, reason);
                }
            }
        }
    }

    // Hard invariant scan (no unmarked mid-sentence-initial turn): needs the
    // run-assembly engine's turns + `continues_previous` facts (task 4.1).
    // The legacy path has no continuation facts — the scan activates when the
    // gate is switched to the engine path; until then it is reported, not run.
    eprintln!("GATE: invariant scan SKIPPED (legacy path carries no continuation facts; activates with task 4.1)");

    eprintln!(
        "GATE SUMMARY: {} passed, {} known-limitation, {} FAILED of {} entries; failed: {:?}",
        passed,
        limited,
        failed.len(),
        fixture.entries.len(),
        failed
    );
    assert!(
        failed.is_empty(),
        "ear-truth gate FAILED for entries {failed:?} — resolve by passing the engine or user-signed KNOWN-LIMITATION"
    );
}

//! Synthetic CI subset of the ear-truth gate (change `hybrid-diarization-engine`,
//! task 1.3): the engine's corroboration and run rules replayed on RECORDED
//! production frame/window arrays from the reference meeting — plain
//! `cargo test`, no audio, no models, no env gate.
//!
//! Arrays recorded by `.tools/extract_synthetic_fixture.py` from one
//! production `frame_masses` pass.

use app_lib::audio::speaker::pyannote_segmentation::{local_labels, FrameMasses, SILENCE_LABEL};
use app_lib::audio::speaker::run_assembly::{
    corroborate_split, derive_pieces, speech_runs, MODE_FILTER_RADIUS_FRAMES, SPEECH_GATE,
    SPLIT_TOLERANCE_SECS,
};
use serde::Deserialize;

const FRAME_SHIFT: f64 = 270.0 / 16000.0;

#[derive(Deserialize)]
struct Fixture {
    cases: std::collections::HashMap<String, Case>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Case {
    Windows(WindowCase),
    Frames(FramesCase),
}

#[derive(Deserialize)]
struct WindowCase {
    window_starts: Vec<f64>,
    window_tracks: Vec<Vec<u8>>,
}

#[derive(Deserialize)]
struct FramesCase {
    start_frame: usize,
    frames: Vec<FrameMasses>,
}

fn load() -> Fixture {
    serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/engine_synthetic_cde5c264.json"
        ))
        .expect("read synthetic fixture"),
    )
    .expect("parse synthetic fixture")
}

#[test]
fn corroborated_change_at_34s_is_accepted() {
    let f = load();
    let Case::Windows(c) = &f.cases["corroborated_34s"] else {
        panic!("wrong case shape");
    };
    let tracks: Vec<(f64, Vec<u8>)> = c
        .window_starts
        .iter()
        .cloned()
        .zip(c.window_tracks.iter().cloned())
        .collect();
    // The 34.66s change is the strongest signal in the meeting: ALL eight
    // covering windows show it. Both adjacent windows must corroborate.
    for t in [34.66, 34.7] {
        assert!(
            corroborate_split(
                &tracks,
                t,
                FRAME_SHIFT,
                SPLIT_TOLERANCE_SECS,
                MODE_FILTER_RADIUS_FRAMES
            ),
            "the corroborated 34.66s change must be accepted (t={t})"
        );
    }
    // A time with no nearby event in either window is rejected.
    assert!(!corroborate_split(
        &tracks,
        33.0,
        FRAME_SHIFT,
        SPLIT_TOLERANCE_SECS,
        MODE_FILTER_RADIUS_FRAMES
    ));
}

#[test]
fn seam_artifact_at_30s_is_rejected() {
    let f = load();
    let Case::Windows(c) = &f.cases["seam_rejected_30s"] else {
        panic!("wrong case shape");
    };
    let tracks: Vec<(f64, Vec<u8>)> = c
        .window_starts
        .iter()
        .cloned()
        .zip(c.window_tracks.iter().cloned())
        .collect();
    // The merged argmax track flips at 30.004s, but EVERY window decodes
    // 26.4–34.66s as continuous single-speaker speech — the flip is a
    // last-writer-wins seam permutation and must NOT split.
    assert!(
        !corroborate_split(
            &tracks,
            30.004,
            FRAME_SHIFT,
            SPLIT_TOLERANCE_SECS,
            MODE_FILTER_RADIUS_FRAMES
        ),
        "the 30.004s seam artifact must be rejected on the recorded windows"
    );
}

#[test]
fn okay_interjection_runs_match_recorded_structure() {
    let f = load();
    let Case::Frames(c) = &f.cases["runs_okay_39s"] else {
        panic!("wrong case shape");
    };
    let labels = local_labels(&c.frames, SPEECH_GATE);
    // Sanity: the recorded slice carries speech and silence.
    assert!(labels.iter().any(|&l| l != SILENCE_LABEL));
    assert!(labels.iter().any(|&l| l == SILENCE_LABEL));
    let runs = speech_runs(&c.frames, FRAME_SHIFT);
    // The recorded slice contains the 39.00–39.93s interjection run
    // (silence-delimited, sub-floor) plus surrounding speech/silence.
    let interjection = runs
        .iter()
        .find(|r| {
            let s = (c.start_frame + r.start_frame) as f64 * FRAME_SHIFT;
            let e = (c.start_frame + r.end_frame) as f64 * FRAME_SHIFT;
            (s - 39.0).abs() < 0.2 && (e - 39.93).abs() < 0.3
        })
        .expect("the 39.0–39.93s interjection run exists in the recorded slice");
    let dur = (interjection.end_frame - interjection.start_frame) as f64 * FRAME_SHIFT;
    assert!(
        dur < 1.5,
        "the interjection is a sub-floor piece (attachment/promotion territory), got {dur:.2}s"
    );
    // And pieces derive for the slice without erroring: the interjection run
    // carries no corroborated label change, so it stays one piece. Piece
    // times are slice-local; add the slice offset for meeting time.
    let offset = c.start_frame as f64 * FRAME_SHIFT;
    let pieces = derive_pieces(
        &labels,
        &runs,
        &[], // no window tracks in this slice: split candidates default to uncorroborated
        FRAME_SHIFT,
        MODE_FILTER_RADIUS_FRAMES,
    );
    let interjection_pieces = pieces
        .iter()
        .filter(|p| {
            let s = p.start_secs + offset;
            let e = p.end_secs + offset;
            s >= 39.0 - 0.2 && e <= 39.93 + 0.3
        })
        .count();
    assert_eq!(
        interjection_pieces, 1,
        "the interjection is exactly one piece: {pieces:?}"
    );
}

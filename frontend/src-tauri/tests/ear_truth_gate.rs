//! Ear-truth fixture gate (change `hybrid-diarization-engine`, tasks 1.2/1.4).
//!
//! MEETIFY_LIVE_DIAG=1 cargo test --test ear_truth_gate -- --ignored --nocapture
//! (run via `openspec/changes/hybrid-diarization-engine/run_gate.bat`, which
//! records every run under that change's `gate-runs/`).
//!
//! Runs the production run-assembly engine (task 4.1 path) on the real
//! meeting audio and asserts every entry in
//! `tests/fixtures/ear_truth_cde5c264.json`, plus the hard invariant scan
//! (every mid-sentence-initial turn must carry `continues_previous = true`)
//! over the ENTIRE meeting output.
//!
//! Semantics: `single_voice` = every turn overlapping the span by >0.25s
//! carries the same label (silence-delimited same-speaker boundaries are not
//! violations — the ear attests voices, not turn units). `voice_change_at` =
//! exactly one label change inside the span, within the pinned tolerance of
//! `change_at_s`.

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use app_lib::audio::speaker::pyannote_segmentation::PyannoteSegmentation;
use app_lib::audio::speaker::run_assembly::{
    align_rows_to_turns, group_fragments_by_turn, is_mid_sentence_start, RowIn, TurnSpan,
};
use app_lib::audio::speaker::run_engine;
use serde::Deserialize;

const MODELS_DIR: &str = ".meetily-models";
const AUDIO: &str =
    "Music/meetily-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";
/// The meeting's resolved max_speakers override (fixture pins 3 clusters).
const MEETING_CAP: usize = 3;
/// The meeting's configured merge threshold.
const MERGE_THRESHOLD: f32 = 0.65;

#[derive(Deserialize)]
struct Fixture {
    #[serde(default)]
    meeting: String,
    entries: Vec<Entry>,
    #[serde(default)]
    known_limitations: Vec<String>,
    /// Auditable waiver records: every known_limitations id MUST carry a
    /// complete record (user confirmation date + reason) — linted by
    /// `ear_truth_fixture_lint` in plain `cargo test`.
    #[serde(default)]
    amendments: std::collections::BTreeMap<String, Amendment>,
}

#[derive(Deserialize)]
struct Amendment {
    user_confirmed: String,
    reason: String,
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

/// One derived turn: label + span + engine continuation fact + aligned text.
struct Turn {
    start: f64,
    end: f64,
    label: u32,
    continues_previous: bool,
    text: String,
}

enum Verdict {
    Pass,
    Fail(String),
}

fn check_single_voice(turns: &[Turn], e: &Entry) -> Verdict {
    let mut overlapping: Vec<(f64, f64, u32)> = Vec::new();
    for t in turns {
        let ov = t.end.min(e.end_s) - t.start.max(e.start_s);
        if ov > 0.25 {
            overlapping.push((t.start, t.end, t.label));
        }
    }
    if overlapping.is_empty() {
        return Verdict::Fail("no turns overlap the span".into());
    }
    let first = overlapping[0].2;
    let mismatched: Vec<String> = overlapping
        .iter()
        .filter(|(_, _, l)| *l != first)
        .map(|(s, en, l)| format!("{}-{}:{}", s, en, l))
        .collect();
    if mismatched.is_empty() {
        Verdict::Pass
    } else {
        Verdict::Fail(format!(
            "label flip inside single-voice span: expected {first} everywhere, found {}",
            mismatched.join(", ")
        ))
    }
}

fn check_voice_change(turns: &[Turn], e: &Entry) -> Verdict {
    let Some(change_at) = e.change_at_s else {
        return Verdict::Fail("voice_change_at entry missing change_at_s".into());
    };
    let tol = e.tolerance_s.unwrap_or(0.5);
    let mut changes: Vec<(f64, u32, u32)> = Vec::new();
    for w in turns.windows(2) {
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

/// `multi_voice`: at least one label change inside the span (count not
/// attested — the user's "two people trading" answers).
fn check_multi_voice(turns: &[Turn], e: &Entry) -> Verdict {
    let changes = turns
        .windows(2)
        .filter(|w| w[0].label != w[1].label && w[1].start >= e.start_s && w[1].start <= e.end_s)
        .count();
    if changes >= 1 {
        Verdict::Pass
    } else {
        Verdict::Fail("no label change inside the span".into())
    }
}

/// Marker correctness on `voice_change_at` entries: a turn at the pinned
/// boundary is DEFECTIVE only when it is marked as continuation while its
/// text begins a fresh sentence (a lying marker). A voice change that cuts a
/// shared whisper row mid-sentence is legitimately marked — the text does
/// continue the sentence (hard invariant).
fn check_marker_false(turns: &[Turn], e: &Entry) -> Option<String> {
    let change_at = e.change_at_s?;
    let following = turns
        .iter()
        .find(|t| t.start >= change_at - 0.01 && t.start <= change_at + 2.0);
    match following {
        Some(t) if t.continues_previous && !is_mid_sentence_start(&t.text) => Some(format!(
            "turn at {:.2}s marked as continuation but its text begins a fresh sentence: {:?}",
            t.start,
            t.text.chars().take(40).collect::<String>()
        )),
        _ => None,
    }
}

/// Plain `cargo test` (no audio/models/env): every KNOWN-LIMITATION id must
/// carry a complete, auditable amendment record, and every amendment record
/// must belong to a listed or existing entry.
#[test]
fn ear_truth_fixture_lint() {
    let fixture: Fixture = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ear_truth_cde5c264.json"
        ))
        .expect("read fixture JSON"),
    )
    .expect("parse fixture JSON");
    let ids: Vec<&String> = fixture.entries.iter().map(|e| &e.id).collect();
    for id in &fixture.known_limitations {
        assert!(
            ids.contains(&id),
            "known_limitations lists unknown entry {id}"
        );
        let record = fixture
            .amendments
            .get(id)
            .unwrap_or_else(|| panic!("KNOWN-LIMITATION {id} has no amendments record"));
        assert!(
            !record.user_confirmed.is_empty() && !record.reason.is_empty(),
            "amendment record for {id} is incomplete (needs user_confirmed + reason)"
        );
        assert!(
            record.user_confirmed.len() == 10
                && record.user_confirmed.as_bytes()[4] == b'-'
                && record.user_confirmed.as_bytes()[7] == b'-',
            "amendment record for {id}: user_confirmed must be a YYYY-MM-DD date, got {:?}",
            record.user_confirmed
        );
    }
    for id in fixture.amendments.keys() {
        assert!(
            fixture.known_limitations.contains(id) || ids.contains(&id),
            "amendments record for {id} references an unknown entry"
        );
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

    // Transcript rows: text spans (textless-run detection) + text (invariant
    // scan). Token-less → proportional alignment (this meeting predates
    // token timestamps).
    let transcript_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(std::path::Path::new(&audio_path)
            .parent().unwrap().join("transcripts.json"))
        .expect("read transcripts.json"),
    )
    .expect("parse transcripts.json");
    let rows = transcript_json
        .get("segments")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let text_spans: Vec<(f64, f64)> = rows
        .iter()
        .filter_map(|r| {
            let a = r.get("audio_start_time")?.as_f64()?;
            let b = r.get("audio_end_time")?.as_f64()?;
            Some((a, b))
        })
        .collect();
    let row_ins: Vec<RowIn> = rows
        .iter()
        .filter_map(|r| {
            Some(RowIn {
                text: r.get("text")?.as_str()?.to_string(),
                start_secs: r.get("audio_start_time")?.as_f64()?,
                end_secs: r.get("audio_end_time")?.as_f64()?,
                tokens: vec![],
            })
        })
        .collect();
    eprintln!(
        "GATE: {} transcript rows as text spans",
        text_spans.len()
    );

    let models_dir = format!("{home}/{MODELS_DIR}");
    let pya = PyannoteSegmentation::new(&format!("{models_dir}/pyannote-segmentation.onnx"))
        .expect("pyannote segmentation model");
    let extractor = NemoEmbeddingExtractor::new(&format!(
        "{}/{}",
        models_dir,
        app_lib::audio::speaker::model_download::embedding_filename()
    ))
    .expect("embedding model");

    // Frame-mass cache: the pyannote pass costs ~16 min; engine-logic
    // iterations reuse the recorded output (identical to re-running inference
    // with the same models and geometry).
    let cache_path = std::path::Path::new(&audio_path)
        .parent()
        .unwrap()
        .join("gate_frame_masses.json");
    // Enrolled references: seeded by badge renames (the rename flow relinks
    // meeting cluster embeddings to named speakers). Absent/empty pool → the
    // consultation rule is inert and the gate runs on meeting-internal
    // clusters only.
    let db_path = std::path::Path::new(&home)
        .join("AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite");
    let references: Vec<(String, Vec<f32>)> = if db_path.exists() {
        match sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .read_only(true)
                    .filename(&db_path),
            )
            .await
        {
            Ok(pool) => {
                let refs =
                    app_lib::database::repositories::speaker::SpeakerRepository::list_stamped_embeddings(&pool)
                        .await
                        .unwrap_or_default();
                refs
            }
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };
    eprintln!("GATE: {} enrolled reference voice(s)", references.len());

    let t0 = std::time::Instant::now();
    let prov = app_lib::audio::speaker::pyannote_segmentation::cache_provenance(
        std::path::Path::new(&format!("{models_dir}/pyannote-segmentation.onnx")),
    )
    .expect("cache provenance (model file)");
    let fm = match app_lib::audio::speaker::pyannote_segmentation::FrameMassesOutput::load(
        &cache_path,
        &prov,
    ) {
        Ok(fm) => {
            eprintln!("GATE: frame masses loaded from cache in {:.1}s", t0.elapsed().as_secs_f64());
            fm
        }
        Err(_) => {
            let fm = pya.frame_masses(&samples).expect("frame masses");
            eprintln!(
                "GATE: pyannote pass in {:.1}s — caching to {}",
                t0.elapsed().as_secs_f64(),
                cache_path.display()
            );
            fm.save(&cache_path, &prov).expect("save frame cache");
            fm
        }
    };
    let out = run_engine::derive_turns_from_masses(
        &fm,
        &extractor,
        &samples,
        &text_spans,
        MERGE_THRESHOLD,
        MEETING_CAP,
        &references,
    )
    .expect("engine run");
    eprintln!(
        "GATE: engine derived {} turns, {} clusters in {:.1}s",
        out.turns.len(),
        out.centroids.len(),
        t0.elapsed().as_secs_f64()
    );
    // Optional centroid dump (enrollment seeding): MEETIFY_CENTROID_DUMP=path
    // writes the final cluster centroids as JSON for the seeding script.
    if let Some(dump_path) = std::env::var_os("MEETIFY_CENTROID_DUMP") {
        let dump: std::collections::BTreeMap<String, Vec<f32>> = out
            .centroids
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        std::fs::write(&dump_path, serde_json::to_string(&dump).expect("serialize centroids"))
            .expect("write centroid dump");
        eprintln!("GATE: centroids dumped to {}", dump_path.display());
    }
    for t in &out.turns {
        eprintln!(
            "TURN {:9.2}-{:.2} sp{}{}{}",
            t.start_seconds,
            t.end_seconds,
            t.speaker_id,
            if t.continues_previous { " cont" } else { "" },
            if t.low_confidence { " lowconf" } else { "" },
        );
    }

    // Align text for the invariant scan (proportional split for token-less
    // rows — accepted limitation; content preservation holds regardless).
    let turn_spans: Vec<TurnSpan> = out
        .turns
        .iter()
        .map(|t| TurnSpan {
            start_secs: t.start_seconds,
            end_secs: t.end_seconds,
            cluster: t.speaker_id as usize,
        })
        .collect();
    let fragments = align_rows_to_turns(&row_ins, &turn_spans);
    let aligned = group_fragments_by_turn(&turn_spans, &fragments);
    let turns: Vec<Turn> = out
        .turns
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let text = aligned.get(i).map(|a| a.text.clone()).unwrap_or_default();
            Turn {
                start: t.start_seconds,
                end: t.end_seconds,
                label: t.speaker_id,
                // The persisted semantics (what the UI renders).
                continues_previous: run_engine::effective_continuation(
                    t.continues_previous,
                    &text,
                ),
                text,
            }
        })
        .collect();

    let mut failed: Vec<String> = Vec::new();
    let mut passed = 0usize;
    let mut limited = 0usize;
    for e in &fixture.entries {
        let mut verdict = match e.kind.as_str() {
            "single_voice" => check_single_voice(&turns, e),
            "voice_change_at" => check_voice_change(&turns, e),
            "multi_voice" => check_multi_voice(&turns, e),
            other => Verdict::Fail(format!("unknown kind {other}")),
        };
        if let Verdict::Pass = verdict {
            if let Some(reason) = check_marker_false(&turns, e) {
                verdict = Verdict::Fail(reason);
            }
        }
        let tag = if e.hold_out { " [hold-out]" } else { "" };
        match verdict {
            Verdict::Pass => {
                passed += 1;
                eprintln!("PASS{} {}", tag, e.id);
            }
            Verdict::Fail(reason) => {
                let listed = fixture.known_limitations.iter().any(|k| k == &e.id);
                let record = fixture.amendments.get(&e.id);
                let waived = listed
                    && record
                        .map(|a| !a.user_confirmed.is_empty() && !a.reason.is_empty())
                        .unwrap_or(false);
                if waived {
                    limited += 1;
                    let a = record.expect("checked");
                    eprintln!(
                        "AMENDED({}) {} — {} | {}",
                        a.user_confirmed, e.id, a.reason, reason
                    );
                } else if listed {
                    failed.push(e.id.clone());
                    eprintln!(
                        "FAIL{} {} — {} (in known_limitations but missing a complete amendments record)",
                        tag, e.id, reason
                    );
                } else {
                    failed.push(e.id.clone());
                    eprintln!("FAIL{} {} — {}", tag, e.id, reason);
                }
            }
        }
    }

    // HARD INVARIANT over the ENTIRE meeting output: every turn whose text
    // begins mid-sentence (lowercase-initial after punct strip) must carry
    // the continuation fact. Production stamps
    // `effective_continuation(engine_flag, first_text)`; the gate recomputes
    // the same rule and fails on any turn presented as a fresh start while
    // beginning mid-sentence.
    let mut violations = 0usize;
    for (i, t) in turns.iter().enumerate() {
        let stamped = run_engine::effective_continuation(t.continues_previous, &t.text);
        if is_mid_sentence_start(&t.text) && !stamped {
            violations += 1;
            eprintln!(
                "INVARIANT VIOLATION: turn {} at {:.2}s starts mid-sentence without continuation fact: {:?}",
                i, t.start, t.text.chars().take(50).collect::<String>()
            );
        }
    }
    eprintln!(
        "GATE: invariant scan: {violations} violation(s) over {} turns",
        turns.len()
    );

    // RENDER-LAYER GATE: the fixture validates the engine's in-memory turns,
    // but the user reads PERSISTED ROWS. Replay the production align →
    // borrow → merge sequence over the real transcript rows and assert the
    // row-level invariants the user demanded: no Unknown badges, no
    // zero-duration slivers, no unmerged same-speaker chops.
    let mut render_failures: Vec<String> = Vec::new();
    {
        use app_lib::audio::speaker::alignment::{
            align_transcripts_with_diarization, DiarizationSegment, TranscriptInput,
        };
        let inputs: Vec<TranscriptInput> = rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                Some(TranscriptInput {
                    id: r
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("row-{i}")),
                    text: r.get("text")?.as_str()?.to_string(),
                    audio_start_ms: (r.get("audio_start_time")?.as_f64()? * 1000.0) as i64,
                    audio_end_ms: (r.get("audio_end_time")?.as_f64()? * 1000.0) as i64,
                    token_words: None,
                })
            })
            .collect();
        let diarization_segs: Vec<DiarizationSegment> = out
            .turns
            .iter()
            .map(|t| DiarizationSegment {
                start_ms: (t.start_seconds * 1000.0) as i64,
                end_ms: (t.end_seconds * 1000.0) as i64,
                speaker_id: t.speaker_id,
            })
            .collect();
        let mut aligned = align_transcripts_with_diarization(inputs, &diarization_segs);
        let fragment_count = aligned.len();
        let unknown_before = aligned
            .iter()
            .filter(|s| s.speaker == "Unknown Speaker")
            .count();
        let cap = app_lib::audio::speaker::commands::GAP_BORROW_MAX_MS;
        app_lib::audio::speaker::commands::assign_engine_gap_fragments(
            &mut aligned,
            &out.turns,
            cap,
        );
        let unknown_after = aligned
            .iter()
            .filter(|s| s.speaker == "Unknown Speaker")
            .count();
        // The design leaves far orphans (beyond the cap) Unknown by choice;
        // an Unknown WITHIN the cap of a turn edge is a borrow failure.
        let unknown_within_cap = aligned
            .iter()
            .filter(|s| s.speaker == "Unknown Speaker")
            .filter(|s| {
                let mid = (s.audio_start_ms + s.audio_end_ms) / 2;
                app_lib::audio::speaker::commands::nearest_turn_span(&diarization_segs, mid)
                    .map(|(d, _)| d <= cap)
                    .unwrap_or(false)
            })
            .count();
        let merged = app_lib::audio::speaker::commands::merge_same_label_fragments(aligned);
        let zero_dur = merged
            .iter()
            .filter(|s| s.audio_end_ms <= s.audio_start_ms)
            .count();
        let unmerged = merged
            .windows(2)
            .filter(|w| {
                w[0].original_id == w[1].original_id && w[0].speaker == w[1].speaker
            })
            .count();
        eprintln!(
            "RENDER: {} rows in → {} fragments aligned ({} Unknown → {} after borrow, {} within cap) → {} merged rows; zero-dur {}, unmerged same-label pairs {}",
            rows.len(),
            fragment_count,
            unknown_before,
            unknown_after,
            unknown_within_cap,
            merged.len(),
            zero_dur,
            unmerged,
        );
        if unknown_within_cap > 0 {
            render_failures.push(format!(
                "{unknown_within_cap} Unknown Speaker fragments remain WITHIN the borrow cap of a turn"
            ));
        }
        // RENDER-TEXT acceptance (task 4.3): the rescue's user-visible win —
        // the fragments covering the rescued span carry the rescuing turn's
        // label (the "Oh, man" words land under Cynthia, not the borrowed
        // Carlos badge).
        {
            // interior of the rescued span (ear: "Oh, man" ≈15.8) — avoids
            // i64-truncation edges at the turn boundary itself
            let rescue_span = (15_700i64, 16_000i64);
            let expected = out
                .turns
                .iter()
                .find(|t| {
                    (t.start_seconds * 1000.0) as i64 <= rescue_span.0
                        && (t.end_seconds * 1000.0) as i64 >= rescue_span.1
                })
                .map(|t| format!("Speaker {}", t.speaker_id));
            let covering: Vec<&app_lib::audio::speaker::alignment::AlignedSegment> = merged
                .iter()
                .filter(|s| s.audio_start_ms < rescue_span.1 && s.audio_end_ms > rescue_span.0)
                .collect();
            let labels: Vec<&str> = covering.iter().map(|s| s.speaker.as_str()).collect();
            let ok = !covering.is_empty()
                && expected
                    .as_ref()
                    .map(|exp| labels.iter().all(|l| *l == exp.as_str()))
                    .unwrap_or(false);
            eprintln!(
                "RENDER-TEXT span {rescue_span:?} -> {} fragment(s), labels {labels:?}, expected {expected:?}: {}",
                covering.len(),
                if ok { "OK" } else { "MISMATCH" }
            );
            if !ok {
                render_failures.push(format!(
                    "render-text acceptance failed: span [15.64,16.12] labels {labels:?}, expected {expected:?}"
                ));
            }
        }
        if zero_dur > 0 {
            render_failures.push(format!("{zero_dur} zero-duration rows"));
        }
        if unmerged > 0 {
            render_failures.push(format!(
                "{unmerged} consecutive same-label same-row fragment pairs survived the merge"
            ));
        }
        if !render_failures.is_empty() {
            for f in &render_failures {
                eprintln!("RENDER FAILURE: {f}");
            }
        }
    }

    eprintln!(
        "GATE SUMMARY: {} passed, {} known-limitation, {} FAILED of {} entries; failed: {:?}",
        passed,
        limited,
        failed.len(),
        fixture.entries.len(),
        failed
    );
    assert!(
        failed.is_empty() && violations == 0 && render_failures.is_empty(),
        "ear-truth gate FAILED for entries {failed:?}, {violations} invariant violations, {} render failures — resolve by passing the engine or user-signed KNOWN-LIMITATION",
        render_failures.len()
    );
}

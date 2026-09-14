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

// ---------------------------------------------------------------------------
// Render-fixture snapshot (change `no-split-sentences`, tasks 1.1–1.4): the
// live DB mutates under every Speakers/Enhance run, so the render replay is
// pinned to a snapshot of the input rows and the live DB is only cross-checked.
// ---------------------------------------------------------------------------

/// One snapshotted transcript row (production `transcripts` columns).
#[derive(Deserialize)]
struct RenderRow {
    id: String,
    text: String,
    start_ms: i64,
    end_ms: i64,
    /// Raw token-timestamps JSON as stored, or None when NULL.
    #[serde(default)]
    token_timestamps: Option<String>,
}

#[derive(Deserialize)]
struct RenderFixture {
    /// FULL meeting id (exact-equality lookup; never a LIKE fragment).
    meeting: String,
    #[serde(default)]
    captured: String,
    row_sha256: String,
    rows: Vec<RenderRow>,
    /// Ear-entry ids this snapshot is allowed to report as known limitations
    /// (each must carry a complete amendments record).
    #[serde(default)]
    known_limitations: Vec<String>,
    /// Auditable waiver records — mirrors the ear-entry amendment mechanism.
    #[serde(default)]
    amendments: std::collections::BTreeMap<String, Amendment>,
}

/// Canonical SHA-256 over the ordered rows — MUST stay in sync with
/// `openspec/changes/no-split-sentences/tools/snapshot_fixture.py` (rows
/// ordered by (start_ms, end_ms, id); per row `id \x1f text \x1f start_ms
/// \x1f end_ms \x1f token_json_or_empty`, rows joined by \x1e).
fn rows_sha256(rows: &[RenderRow]) -> String {
    use sha2::{Digest, Sha256};
    let mut sorted: Vec<&RenderRow> = rows.iter().collect();
    sorted.sort_by(|a, b| {
        (a.start_ms, a.end_ms, a.id.as_str()).cmp(&(b.start_ms, b.end_ms, b.id.as_str()))
    });
    let canon: Vec<String> = sorted
        .iter()
        .map(|r| {
            format!(
                "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
                r.id,
                r.text,
                r.start_ms,
                r.end_ms,
                r.token_timestamps.as_deref().unwrap_or("")
            )
        })
        .collect();
    let mut h = Sha256::new();
    h.update(canon.join("\u{1e}").as_bytes());
    format!("{:x}", h.finalize())
}

/// Sentence-terminal test for the fracture predicate (task 1.3): closing
/// quotes/brackets trimmed, then a `.?!` / full-width `。？！` / ellipsis.
fn ends_with_sentence_terminal(text: &str) -> bool {
    text.trim_end()
        .trim_end_matches(|c| {
            matches!(c, '"' | '\'' | ')' | ']' | '}' | '”' | '’' | '»' | '」' | '』')
        })
        .chars()
        .last()
        .map_or(false, |c| {
            matches!(c, '.' | '?' | '!' | '。' | '？' | '！' | '…')
        })
}

/// Normalized token list for the duplicate predicate (task 1.4):
/// detokenize → lowercase → non-alphanumeric → space → collapse whitespace.
fn norm_tokens(text: &str) -> Vec<String> {
    app_lib::audio::speaker::turns::detokenize(text)
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// Longest common CONTIGUOUS token subsequence length (a shared chunk, not a
/// scattered LCS — the motivating cluster shares one re-decoded stretch).
fn longest_shared_chunk(a: &[String], b: &[String]) -> usize {
    let mut best = 0usize;
    let mut dp = vec![0usize; b.len() + 1];
    for wa in a {
        let mut prev_diag = 0usize;
        for (j, wb) in b.iter().enumerate() {
            let tmp = dp[j + 1];
            if wa == wb {
                dp[j + 1] = prev_diag + 1;
                best = best.max(dp[j + 1]);
            } else {
                dp[j + 1] = 0; // contiguity: reset, never carry a scattered best
            }
            prev_diag = tmp;
        }
    }
    best
}

/// Record a render-level failure under the amendment waiver path: a kind
/// listed in `known_limitations` WITH a complete amendment record is
/// printed AMENDED(<date>, <reason>) and does not fail the gate; listed
/// without a record fails loudly; unlisted fails plainly. (The fracture scan
/// no longer routes through here — since the 2026-09-09 ear decree every
/// cross-badge fracture fails hard, with no waiver kind.)
fn record_render_failure(
    render_failures: &mut Vec<String>,
    fx: &RenderFixture,
    kind: &str,
    msg: String,
) {
    let listed = fx.known_limitations.iter().any(|k| k == kind);
    let record = fx.amendments.get(kind);
    let complete = record
        .map(|a| !a.user_confirmed.is_empty() && !a.reason.is_empty())
        .unwrap_or(false);
    if listed && complete {
        let a = record.expect("checked");
        eprintln!("AMENDED({}) {kind} — {} | {msg}", a.user_confirmed, a.reason);
        return;
    }
    if listed {
        render_failures.push(format!(
            "{kind}: {msg} (in known_limitations but missing a complete amendments record)"
        ));
    } else {
        render_failures.push(format!("{kind}: {msg}"));
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
    // Samples cache (both-bars iteration, 2026-09-09): the decode costs
    // ~6.5 min per gate run; the raw f32 dump beside the audio is the exact
    // `decode_audio_file().to_whisper_format()` result, so loading it is
    // equivalent (meta file pins the sample count).
    let samples = {
        let dir = std::path::Path::new(&audio_path).parent().unwrap().to_path_buf();
        let cache = dir.join("samples_16k.f32");
        let meta = dir.join("samples_16k.meta.json");
        if cache.exists() && meta.exists() {
            let bytes = std::fs::read(&cache).expect("read samples cache");
            let n: usize =
                serde_json::from_str(&std::fs::read_to_string(&meta).expect("samples meta"))
                    .expect("parse samples meta");
            let s: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            assert_eq!(s.len(), n, "samples cache/meta mismatch");
            eprintln!("GATE: samples loaded from cache ({n} = {:.1}s)", n as f64 / 16_000.0);
            s
        } else {
            let decoded =
                app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
                    .expect("decode audio");
            let s = decoded.to_whisper_format();
            let bytes: Vec<u8> = s.iter().flat_map(|v| v.to_le_bytes()).collect();
            std::fs::write(&cache, &bytes).expect("write samples cache");
            std::fs::write(&meta, s.len().to_string()).expect("write samples meta");
            eprintln!("GATE: decoded {:.1}s — samples cached to {}", decoded.duration_seconds, cache.display());
            s
        }
    };

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
    // the continuation fact.
    //
    // KNOWN TAUTOLOGY (annotated per no-split-sentences task 1.4): production
    // stamps `effective_continuation(engine_flag, first_text)`, which is
    // `engine_flag || is_mid_sentence_start(first_text)` — TRUE whenever this
    // scan's own antecedent holds. The "0 violations" below is therefore
    // vacuous and MUST NOT be cited as evidence of sentence health; the
    // cross-badge fracture scan in the render block is the substantive check.
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
        "GATE: invariant scan (TAUTOLOGICAL — not evidence): {violations} violation(s) over {} turns",
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
        // no-split-sentences task 1.2: the replay input is the SNAPSHOT
        // fixture (`cde5c264_transcripts.json`) — the live DB mutates under
        // every Speakers/Enhance run, so RED/GREEN must be pinned to bytes.
        // The live DB is still cross-checked: exact-equality meeting lookup
        // (exactly one match), row count, canonical hash. Any mismatch is a
        // loud "fixture drifted — re-pin"; 0 rows can never pass vacuously.
        let render_fixture: RenderFixture = serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/cde5c264_transcripts.json"
            ))
            .expect("read cde5c264_transcripts.json"),
        )
        .expect("parse cde5c264_transcripts.json");
        assert_eq!(
            rows_sha256(&render_fixture.rows),
            render_fixture.row_sha256,
            "render fixture self-check failed: hash over its own rows != row_sha256"
        );
        assert!(
            !render_fixture.rows.is_empty(),
            "render fixture has 0 rows — never a vacuous green"
        );
        let render_pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .read_only(true)
                    .filename(&db_path),
            )
            .await
            .expect("render replay: open prod DB read-only");
        use sqlx::Row;
        let meeting_hits: Vec<String> = sqlx::query("SELECT id FROM meetings WHERE id = ?1")
            .bind(&render_fixture.meeting)
            .fetch_all(&render_pool)
            .await
            .expect("render replay: exact meeting lookup")
            .iter()
            .map(|r| r.get::<String, _>("id"))
            .collect();
        assert_eq!(
            meeting_hits.len(),
            1,
            "render replay: expected exactly one meeting with id {:?}, found {}",
            render_fixture.meeting,
            meeting_hits.len()
        );
        // align-from-immutable-source task 3.2: the cross-check target is the
        // pipeline's ACTUAL INPUT — the immutable `transcript_sources` table —
        // since the replay input becomes the pipeline's input post-split. The
        // rendering rows (`transcripts`) are engine OUTPUT and no longer pin
        // the replay.
        let (has_sources,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'transcript_sources'",
        )
        .fetch_one(&render_pool)
        .await
        .expect("render replay: table lookup");
        assert!(
            has_sources > 0,
            "transcript_sources missing — run the app once so the \
             align-from-immutable-source migration applies, then re-pin"
        );
        let live_rows: Vec<RenderRow> = sqlx::query(
            "SELECT id, transcript, audio_start_time, audio_end_time, token_timestamps \
             FROM transcript_sources WHERE meeting_id = ?1",
        )
        .bind(&render_fixture.meeting)
        .fetch_all(&render_pool)
        .await
        .expect("render replay: read transcript rows")
        .iter()
        .map(|r| RenderRow {
            id: r.get::<String, _>("id"),
            text: r.get::<String, _>("transcript"),
            start_ms: (r.get::<f64, _>("audio_start_time") * 1000.0) as i64,
            end_ms: (r.get::<f64, _>("audio_end_time") * 1000.0) as i64,
            token_timestamps: r.get::<Option<String>, _>("token_timestamps"),
        })
        .collect();
        let live_hash = rows_sha256(&live_rows);
        assert!(
            live_rows.len() == render_fixture.rows.len()
                && live_hash == render_fixture.row_sha256,
            "fixture drifted — re-pin: live DB differs from snapshot \
             cde5c264_transcripts.json (snapshot: {} rows / sha {}, live: {} rows / \
             sha {}). Understand the mutation, then re-run \
             openspec/changes/no-split-sentences/tools/snapshot_fixture.py.",
            render_fixture.rows.len(),
            render_fixture.row_sha256,
            live_rows.len(),
            live_hash
        );
        eprintln!(
            "GATE: render replay pinned to snapshot: {} rows, live DB cross-check matches ({})",
            render_fixture.rows.len(),
            live_hash
        );
        let inputs: Vec<TranscriptInput> = render_fixture
            .rows
            .iter()
            .map(|r| TranscriptInput {
                id: r.id.clone(),
                text: r.text.clone(),
                audio_start_ms: r.start_ms,
                audio_end_ms: r.end_ms,
                token_words: r
                    .token_timestamps
                    .as_deref()
                    .and_then(|json| serde_json::from_str(json).ok()),
            })
            .collect();
        let input_count = inputs.len();
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
        // The no-chop invariant is the merge's OWN guarantee (production runs
        // it right here): after this point resolve_duplicate_clusters absorbs
        // whole rows, which can make surviving fragments of one source row
        // adjacent again — a benign shape the render consolidation (and the
        // persisted DB row) presents as ONE row. Count BEFORE the dupe pass.
        let unmerged = merged
            .windows(2)
            .filter(|w| {
                w[0].original_id == w[1].original_id && w[0].speaker == w[1].speaker
            })
            .count();
        let mut unmerged_pairs: Vec<(i64, i64, i64, i64, String)> = Vec::new();
        for w in merged.windows(2) {
            if w[0].original_id == w[1].original_id && w[0].speaker == w[1].speaker {
                unmerged_pairs.push((
                    w[0].audio_start_ms,
                    w[0].audio_end_ms,
                    w[1].audio_start_ms,
                    w[1].audio_end_ms,
                    format!("{} / {}", w[0].text, w[1].text),
                ));
            }
        }
        // Production resolves duplicate re-transcription clusters between the
        // merge and the persist step (no-split-sentences D4); replay it so the
        // assertions and dump judge the shape persist would actually write.
        let merged = app_lib::audio::speaker::alignment::resolve_duplicate_clusters(merged);
        // Production Step 8 tail: same-speaker consolidation (sentence-aware
        // turn assembly, gap ≤3s) produces the PERSISTED row shape the UI
        // serves. Replay it so the assertions and dump judge what the user
        // actually reads, not the intermediate fragment stage.
        let refs: Vec<app_lib::audio::speaker::turns::RowRef<'_>> = merged
            .iter()
            .map(|r| app_lib::audio::speaker::turns::RowRef {
                speaker: &r.speaker,
                start_ms: r.audio_start_ms,
                end_ms: r.audio_end_ms,
                text: &r.text,
            })
            .collect();
        let groups = app_lib::audio::speaker::turns::assemble_groups(&refs);
        let cons_zero_dur = groups
            .iter()
            .filter(|g| g.turn.end_ms <= g.turn.start_ms)
            .count();
        if cons_zero_dur > 0 {
            record_render_failure(
                &mut render_failures,
                &render_fixture,
                "zero_duration_rows",
                format!("{cons_zero_dur} zero-duration persisted rows"),
            );
        }
        // MEETIFY_RENDER_PRINT=1: dump the persisted rows the UI will show
        // after the Speakers run, plus the user's suspect rule (any segment
        // starting mid-sentence / lowercase) over ALL of them.
        if std::env::var_os("MEETIFY_RENDER_PRINT").is_some() {
            eprintln!("=== PERSISTED ROWS [0s,60s] (align → borrow → merge → consolidate) ===");
            for g in groups.iter().filter(|g| g.turn.start_ms < 60_000) {
                eprintln!(
                    "PERSISTED-ROW [{:>7.2}–{:7.2}] {}: {}",
                    g.turn.start_ms as f64 / 1000.0,
                    g.turn.end_ms as f64 / 1000.0,
                    g.turn.speaker,
                    g.turn.text
                );
            }
            let frag_suspects = merged
                .iter()
                .filter(|r| app_lib::audio::speaker::run_assembly::is_mid_sentence_start(&r.text))
                .count();
            let suspects: Vec<_> = groups
                .iter()
                .filter(|g| {
                    app_lib::audio::speaker::run_assembly::is_mid_sentence_start(&g.turn.text)
                })
                .collect();
            eprintln!(
                "=== SUSPECT SCAN: persisted {} lowercase/mid-sentence-initial row(s) of {} (fragment stage: {frag_suspects} of {}) ===",
                suspects.len(),
                groups.len(),
                merged.len()
            );
            for g in suspects.iter().take(30) {
                eprintln!(
                    "SUSPECT [{:.2}] {}: {}",
                    g.turn.start_ms as f64 / 1000.0,
                    g.turn.speaker,
                    g.turn.text.chars().take(80).collect::<String>()
                );
            }
        }
        let zero_dur = merged
            .iter()
            .filter(|s| s.audio_end_ms <= s.audio_start_ms)
            .count();
        eprintln!(
            "RENDER: {} DB rows in → {} fragments aligned ({} Unknown → {} after borrow, {} within cap) → {} merged rows; zero-dur {}, unmerged same-label pairs {} (pre-dupe)",
            input_count,
            fragment_count,
            unknown_before,
            unknown_after,
            unknown_within_cap,
            merged.len(),
            zero_dur,
            unmerged,
        );
        if unknown_within_cap > 0 {
            record_render_failure(
                &mut render_failures,
                &render_fixture,
                "unknown_within_cap",
                format!(
                    "{unknown_within_cap} Unknown Speaker fragments remain WITHIN the borrow cap of a turn"
                ),
            );
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
            record_render_failure(
                &mut render_failures,
                &render_fixture,
                "zero_duration_rows",
                format!("{zero_dur} zero-duration rows"),
            );
        }
        if unmerged > 0 {
            for (a0, b0, a1, b1, text) in &unmerged_pairs {
                eprintln!(
                    "UNMERGED PAIR [{a0}-{b0}] | [{a1}-{b1}]: {text}"
                );
            }
            record_render_failure(
                &mut render_failures,
                &render_fixture,
                "unmerged_fragments",
                format!(
                    "{unmerged} consecutive same-label same-row fragment pairs survived the merge"
                ),
            );
        }

        // no-split-sentences task 1.3 — cross-badge FRACTURE scan over the
        // persisted shape. Row i>0 fractures iff it begins mid-sentence, its
        // badge differs from the previous persisted row, and that row does
        // not end in sentence-terminal punctuation (closing quotes/brackets
        // trimmed). Row 0 is exempt; a same-badge lowercase onset is ASR
        // style, not a fracture (consolidation already merged same-badge
        // ≤3s neighbours, so a surviving same-badge adjacency is a >3s
        // resume). User ear decree (2026-09-09): a voice does NOT change
        // mid-sentence — the bounded-run tail waiver class is RETIRED, so
        // EVERY fracture is a defect and fails hard.
        let mut fractures: Vec<String> = Vec::new();
        for (i, g) in groups.iter().enumerate() {
            if i == 0 {
                continue;
            }
            let prev = &groups[i - 1];
            if g.turn.speaker != prev.turn.speaker
                && is_mid_sentence_start(&g.turn.text)
                && !ends_with_sentence_terminal(&prev.turn.text)
            {
                fractures.push(format!(
                    "[{:>7.2}] {} {:?} continues [{:>7.2}] {} {:?}",
                    g.turn.start_ms as f64 / 1000.0,
                    g.turn.speaker,
                    g.turn.text.chars().take(60).collect::<String>(),
                    prev.turn.start_ms as f64 / 1000.0,
                    prev.turn.speaker,
                    prev.turn.text.chars().take(60).collect::<String>(),
                ));
            }
        }
        for f in &fractures {
            eprintln!("FRACTURE: {f}");
        }
        eprintln!(
            "GATE: fracture scan: {} cross-badge fracture(s) of {} persisted rows (all are defects — no waiver class)",
            fractures.len(),
            groups.len()
        );
        for f in &fractures {
            render_failures.push(format!("unexpected cross-badge fracture: {f}"));
        }

        // no-split-sentences task 1.4 — DUPLICATE-CLUSTER scan over a ±10 s
        // window (the motivating cluster is not adjacent in the persisted
        // sequence). Pair predicate: normalized token sequences share a
        // contiguous ≥3-token chunk covering ≥80% of the shorter row, spans
        // disjoint, gap ≤2 s, different badges (or one side Unknown).
        // Matching pairs are union-found into clusters for reporting.
        let unknown_badge = "Unknown Speaker";
        let n = groups.len();
        fn uf_find(parent: &mut Vec<usize>, x: usize) -> usize {
            if parent[x] != x {
                let root = uf_find(parent, parent[x]);
                parent[x] = root;
                root
            } else {
                x
            }
        }
        let mut parent: Vec<usize> = (0..n).collect();
        let mut pair_hits: Vec<String> = Vec::new();
        let mut overlap_pairs: Vec<String> = Vec::new();
        for i in 0..n {
            for j in i + 1..n {
                let a = &groups[i].turn;
                let b = &groups[j].turn;
                if b.start_ms - a.start_ms > 10_000 {
                    break; // groups are time-ordered; nothing further in window
                }
                // overlapping spans = same-audio double-decode suspect:
                // reported and failed, NEVER dropped silently here
                if a.start_ms < b.end_ms && b.start_ms < a.end_ms {
                    overlap_pairs.push(format!(
                        "[{:>7.2}-{:.2}] {} <-> [{:>7.2}-{:.2}] {}",
                        a.start_ms as f64 / 1000.0,
                        a.end_ms as f64 / 1000.0,
                        a.speaker,
                        b.start_ms as f64 / 1000.0,
                        b.end_ms as f64 / 1000.0,
                        b.speaker,
                    ));
                    continue;
                }
                let (first, second) = if a.end_ms <= b.start_ms {
                    (a, b)
                } else {
                    (b, a)
                };
                if second.start_ms - first.end_ms > 2_000 {
                    continue;
                }
                if a.speaker != b.speaker
                    && (a.speaker != unknown_badge || b.speaker != unknown_badge)
                {
                    let ta = norm_tokens(&a.text);
                    let tb = norm_tokens(&b.text);
                    let shared = longest_shared_chunk(&ta, &tb);
                    let shorter = ta.len().min(tb.len());
                    if shared >= 3 && shorter > 0 && shared >= (0.8 * shorter as f64) as usize {
                        let ri = uf_find(&mut parent, i);
                        let rj = uf_find(&mut parent, j);
                        if ri != rj {
                            parent[ri] = rj;
                        }
                        pair_hits.push(format!(
                            "[{:>7.2}] {} {:?} <-> [{:>7.2}] {} {:?} (shared {shared}/{shorter} tokens)",
                            a.start_ms as f64 / 1000.0,
                            a.speaker,
                            a.text.chars().take(60).collect::<String>(),
                            b.start_ms as f64 / 1000.0,
                            b.speaker,
                            b.text.chars().take(60).collect::<String>(),
                        ));
                    }
                }
            }
        }
        let mut cluster_map: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
        for i in 0..n {
            cluster_map.entry(uf_find(&mut parent, i)).or_default().push(i);
        }
        let clusters: Vec<Vec<usize>> = cluster_map
            .into_values()
            .filter(|v| v.len() > 1)
            .collect();
        for (ci, c) in clusters.iter().enumerate() {
            eprintln!("DUPLICATE CLUSTER {} ({} rows):", ci + 1, c.len());
            for i in c {
                eprintln!(
                    "  [{:>7.2}] {}: {}",
                    groups[*i].turn.start_ms as f64 / 1000.0,
                    groups[*i].turn.speaker,
                    groups[*i].turn.text
                );
            }
        }
        for p in &pair_hits {
            eprintln!("DUPLICATE PAIR: {p}");
        }
        eprintln!(
            "GATE: duplicate scan: {} cluster(s) / {} matching pair(s); overlapping-span rows: {} pair(s) (never dropped)",
            clusters.len(),
            pair_hits.len(),
            overlap_pairs.len()
        );
        if !clusters.is_empty() {
            record_render_failure(
                &mut render_failures,
                &render_fixture,
                "duplicate_cluster",
                format!(
                    "{} duplicate re-transcription cluster(s): {:?}",
                    clusters.len(),
                    clusters
                        .iter()
                        .map(|c| {
                            c.iter()
                                .map(|i| format!("[{:.2}]", groups[*i].turn.start_ms as f64 / 1000.0))
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>()
                ),
            );
        }
        if !overlap_pairs.is_empty() {
            render_failures.push(format!(
                "{} overlapping-span row pair(s) — same-audio double-decode suspects (never dropped)",
                overlap_pairs.len()
            ));
        }

        // Churn counters (task 3.1 reports them; threshold decisions deferred).
        {
            let meeting_secs = samples.len() as f64 / 16_000.0;
            let rows_per_min = groups.len() as f64 / (meeting_secs / 60.0);
            let short_rows = groups
                .iter()
                .filter(|g| g.turn.text.split_whitespace().count() <= 2)
                .count();
            eprintln!(
                "GATE: churn: {rows_per_min:.1} persisted rows/minute over {meeting_secs:.0}s; {short_rows} row(s) of <=2 words"
            );
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

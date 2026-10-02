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
const MEETING_DIR: &str = "Meeting 2026-06-22_16-04-01_2026-06-22_14-04";

/// Recording folder resolver: `Music/*-recordings/<meeting>/audio.mp4`.
/// The root's name varies per machine (the literal is a local-only marker
/// and must not appear in code); MEETIFY_RECORDINGS_DIR overrides.
fn resolve_audio() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("MEETIFY_RECORDINGS_DIR") {
        return std::path::Path::new(&dir).join(MEETING_DIR).join("audio.mp4");
    }
    let home = std::env::var("USERPROFILE").expect("USERPROFILE");
    let music = std::path::Path::new(&home).join("Music");
    for entry in std::fs::read_dir(&music).expect("read Music") {
        let p = entry.expect("dir entry").path();
        if p.is_dir() && p.join(MEETING_DIR).join("audio.mp4").exists() {
            return p.join(MEETING_DIR).join("audio.mp4");
        }
    }
    panic!("no *-recordings folder with {MEETING_DIR} under {}", music.display());
}
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
    /// Per-voice text needles for the synthesis assertion (S16): verbatim
    /// from THIS local fixture — never code. Empty for entries that don't
    /// pin stream text.
    #[serde(default)]
    needles: Vec<String>,
    /// Text that must NOT appear in the entry's window (S18): the
    /// fabrication signature the phrase-loop repair exists to remove.
    #[serde(default)]
    absence_needles: Vec<String>,
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

    let audio_path = resolve_audio();
    // Samples cache (both-bars iteration, 2026-09-09): the decode costs
    // ~6.5 min per gate run; the raw f32 dump beside the audio is the exact
    // `decode_audio_file().to_whisper_format()` result, so loading it is
    // equivalent (meta file pins the sample count).
    let samples = {
        // The samples cache carries the SOURCE AUDIO's sha256 so a stale or
        // wrong-meeting cache cannot silently pass (the frame-mass cache
        // already proves its model provenance; the samples were the gap).
        // Old count-only metas are accepted with a loud warning — delete
        // samples_16k.f32 to upgrade the cache to pinned provenance.
        use sha2::{Digest, Sha256};
        let dir = std::path::Path::new(&audio_path).parent().unwrap().to_path_buf();
        let cache = dir.join("samples_16k.f32");
        let meta = dir.join("samples_16k.meta.json");
        let source_sha = {
            let mut h = Sha256::new();
            let mut file = std::fs::File::open(&audio_path).expect("open source audio");
            std::io::copy(&mut file, &mut h).expect("hash source audio");
            format!("{:x}", h.finalize())
        };
        let decode_and_cache = || {
            let decoded =
                app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
                    .expect("decode audio");
            let s = decoded.to_whisper_format();
            let bytes: Vec<u8> = s.iter().flat_map(|v| v.to_le_bytes()).collect();
            std::fs::write(&cache, &bytes).expect("write samples cache");
            let meta_json = serde_json::json!({ "samples": s.len(), "audio_sha256": source_sha });
            std::fs::write(&meta, meta_json.to_string()).expect("write samples meta");
            eprintln!("GATE: decoded {:.1}s — samples cached to {}", decoded.duration_seconds, cache.display());
            s
        };
        if cache.exists() && meta.exists() {
            let meta_json: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&meta).expect("samples meta"),
            )
            .expect("parse samples meta");
            let cached_sha = meta_json.get("audio_sha256").and_then(|v| v.as_str());
            let n: usize = match meta_json.get("samples") {
                Some(v) => v.as_u64().expect("samples count") as usize,
                // legacy count-only meta
                None => serde_json::from_str::<usize>(
                    &std::fs::read_to_string(&meta).expect("samples meta"),
                )
                .unwrap_or_else(|_| {
                    panic!("samples meta is neither pinned JSON nor a count")
                }),
            };
            match cached_sha {
                Some(sha) if sha != source_sha => {
                    eprintln!(
                        "GATE: samples cache audio_sha256 mismatch (cache {sha} != source) — re-decoding"
                    );
                    decode_and_cache()
                }
                Some(_) => {
                    let bytes = std::fs::read(&cache).expect("read samples cache");
                    let s: Vec<f32> = bytes
                        .chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect();
                    assert_eq!(s.len(), n, "samples cache/meta mismatch");
                    eprintln!(
                        "GATE: samples loaded from cache ({n} = {:.1}s, provenance OK)",
                        n as f64 / 16_000.0
                    );
                    s
                }
                None => {
                    let bytes = std::fs::read(&cache).expect("read samples cache");
                    let s: Vec<f32> = bytes
                        .chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect();
                    assert_eq!(s.len(), n, "samples cache/meta mismatch");
                    eprintln!(
                        "GATE: WARNING samples cache has NO audio provenance (legacy meta) — trusting {} samples; delete the cache to re-pin",
                        n
                    );
                    s
                }
            }
        } else {
            decode_and_cache()
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
                    app_lib::database::repositories::speaker::SpeakerRepository::list_enrollment_refs(&pool)
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
             openspec/changes/archive/2026-09-13-no-split-sentences/tools/snapshot_fixture.py.",
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
                sustained_split: t.starts_voice_split,
            })
            .collect();
        let rescue_segs: Vec<DiarizationSegment> = out
            .rescue_seams
            .iter()
            .map(|(s, e, c)| DiarizationSegment {
                start_ms: (s * 1000.0) as i64,
                end_ms: (e * 1000.0) as i64,
                speaker_id: *c,
                sustained_split: false,
            })
            .collect();
        let mut vote_segs: Vec<DiarizationSegment> = out
            .voice_votes
            .iter()
            .map(|(s, e, c)| DiarizationSegment {
                start_ms: (s * 1000.0) as i64,
                end_ms: (e * 1000.0) as i64,
                speaker_id: *c,
                sustained_split: false,
            })
            .collect();
        // Word-wall atom votes (production parity): the live path computes
        // these in commands.rs; the gate replays the same call over the
        // fixture inputs so the render-text pins hold by the same evidence.
        // Synthesis candidates ride out of this scope for the splice below
        // (overlap-stream-retranscription 3.3).
        let mut gate_synthesis_inputs: Vec<run_engine::SpanSynthesisInput> = Vec::new();
        {
            let cent_pairs: Vec<(u32, Vec<f32>)> = out
                .centroids
                .iter()
                .map(|(k, v)| (*k, v.clone()))
                .collect();
            let wall = run_engine::wall_atom_voice_votes(
                &samples,
                &inputs,
                &extractor,
                &cent_pairs,
                &references,
            );
            eprintln!("GATE: +{} word-wall atom voice votes", wall.len());
            // Separation pre-pass (production parity with commands.rs):
            // per-stream votes over the engine's attested overlap spans,
            // with the wall votes for separation-covered atoms dropped. A
            // missing model degrades to the mixture-only channel.
            let separator =
                app_lib::audio::speaker::separation::ConvTasNetSeparator::from_models_dir();
            let separated = separator.as_ref().map(|s| {
                let embed =
                    |a: &[f32]| extractor.extract_embedding(a, run_engine::SAMPLE_RATE);
                let (v, c, synthesis_inputs) = run_engine::separated_stream_voice_votes(
                    &samples,
                    &inputs,
                    &embed,
                    &cent_pairs,
                    &references,
                    s,
                    &out.overlap_spans,
                );
                gate_synthesis_inputs = synthesis_inputs;
                (v, c)
            });
            eprintln!(
                "GATE: separation {} ({} spans)",
                if separated.is_some() { "active" } else { "degraded (no model)" },
                out.overlap_spans.len()
            );
            let wall = run_engine::merge_wall_and_separated(wall, separated);
            vote_segs.extend(wall.iter().map(|(s, e, c)| DiarizationSegment {
                start_ms: (s * 1000.0) as i64,
                end_ms: (e * 1000.0) as i64,
                speaker_id: *c,
                sustained_split: false,
            }));
        }
        let mut aligned =
            align_transcripts_with_diarization(inputs.clone(), &diarization_segs, &rescue_segs, &vote_segs);
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
        // Overlap-stream synthesis replay (overlap-stream-retranscription
        // 3.3): the SAME pure splice production runs after the resolver,
        // with a REAL Whisper engine — the degrade path (engine None) would
        // suppress every synthesized row and the S16 assertion could never
        // pass. Language and model are pinned explicitly by the operator
        // (env): the gate process has no language preference static, and an
        // automatic mode degrades by design.
        let gate_lang = std::env::var("MEETIFY_GATE_LANG").unwrap_or_else(|_| {
            eprintln!("GATE: MEETIFY_GATE_LANG unset — stream decode degrades (no synthesis)");
            String::new()
        });
        let gate_decoder = {
            // The whisper models live in the production app dir (ONNX
            // models live in ~/.meetily-models — a different store).
            let whisper_dir = std::path::Path::new(&home)
                .join("AppData/Roaming/com.meetily.ai/models");
            let engine = app_lib::whisper_engine::WhisperEngine::new_with_models_dir(Some(
                whisper_dir,
            ))
            .expect("gate whisper engine");
            let model = std::env::var("MEETIFY_GATE_WHISPER_MODEL").unwrap_or_default();
            if gate_lang.is_empty() || model.is_empty() {
                None
            } else {
                let mut e = engine;
                // discover + load are async; the gate test is async too —
                // block_in_place is unavailable in #[tokio::test] without
                // multithread runtime, so construction happens via the
                // existing async surface.
                Some((e, model, gate_lang))
            }
        };
        let gate_manual_spans = if db_path.exists() {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    sqlx::sqlite::SqliteConnectOptions::new()
                        .read_only(true)
                        .filename(&db_path),
                )
                .await;
            match pool {
                Ok(p) => app_lib::database::repositories::speaker::SpeakerRepository::list_manual_spans(&p, &render_fixture.meeting).await,
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let mut gate_synthesis: Vec<app_lib::audio::speaker::run_assembly::SpanSynthesis> = Vec::new();
        if let Some((mut engine, model, lang)) = gate_decoder {
            use app_lib::audio::speaker::run_assembly::{SpanSynthesis, StreamEvidence};
            engine.discover_models().await.expect("discover models");
            engine.load_model(&model).await.expect("load gate whisper model");
            // The blocking decode is spawn_blocking-ONLY (tokio panics on a
            // runtime worker — the design's pinned constraint); model load
            // stays on the async surface. The engine MOVES into the blocking
            // closure.
            let inputs = std::mem::take(&mut gate_synthesis_inputs);
            gate_synthesis = tokio::task::spawn_blocking(move || {
                let mut out = Vec::new();
                for input in &inputs {
                    let [Some((c0, m0)), Some((c1, m1))] = input.identity else { continue };
                    let decode = |samples: &[f32]| -> Option<String> {
                        let text = engine.transcribe_span_blocking(samples.to_vec(), &lang)?;
                        let trimmed = text.trim();
                        if !trimmed.chars().any(|c| c.is_alphanumeric()) {
                            return None;
                        }
                        let end_ms = samples.len() as f64
                            / run_engine::SAMPLE_RATE as f64
                            * 1000.0;
                        let report = app_lib::audio::hallucination::audit(trimmed, 0.0, end_ms);
                        if report.is_garbage { None } else { Some(trimmed.to_string()) }
                    };
                    let t0 = decode(&input.streams[0]);
                    let t1 = decode(&input.streams[1]);
                    out.push(SpanSynthesis {
                        span: input.span,
                        streams: [
                            StreamEvidence { cluster: c0, margin: m0, rms_ratio: input.rms_ratio[0], text: t0 },
                            StreamEvidence { cluster: c1, margin: m1, rms_ratio: input.rms_ratio[1], text: t1 },
                        ],
                        covered_atoms: input.covered_atoms.clone(),
                    });
                }
                out
            })
            .await
            .expect("synthesis decode join");
        }
        let (mut merged, gate_synthesized) = app_lib::audio::speaker::commands::apply_overlap_synthesis(
            merged,
            &gate_synthesis,
            &gate_manual_spans,
            &Default::default(),
        );

        // Phrase-loop repair replay (S18, ear round 2026-10-01): the same
        // trigger + windows + gates as the production pass. The mass decode
        // closure consumed the first engine, so the repair decode builds a
        // second engine instance; the S18 assertion below makes a silent
        // degrade a FAILURE, not a skip.
        {
            let mut repair_seed_spans: Vec<(i64, i64)> = merged
                .iter()
                .filter(|r| {
                    !r.synth_atom
                        && app_lib::audio::speaker::run_assembly::is_phrase_loop(&r.text)
                        && (r.audio_end_ms - r.audio_start_ms) as f64 / 1000.0
                            <= app_lib::audio::speaker::run_assembly::OVERLAP_MAX_SPAN_SECS
                })
                .map(|r| (r.audio_start_ms, r.audio_end_ms))
                .collect();
            // stutter promotion (ear ruling 2026-10-02): stood-down spans
            // whose separated decode stuttered re-seed the repair
            repair_seed_spans.extend(
                gate_synthesis
                    .iter()
                    .filter(|s| {
                        s.streams.iter().any(|st| {
                            st.text
                                .as_deref()
                                .map(|t| app_lib::audio::speaker::run_assembly::is_stuttering_decode(t))
                                .unwrap_or(false)
                        })
                    })
                    .map(|s| ((s.span.0 * 1000.0) as i64, (s.span.1 * 1000.0) as i64)),
            );
            let repair_candidates = app_lib::audio::speaker::run_assembly::repair_windows_from_seeds(
                &merged,
                &repair_seed_spans,
                &gate_manual_spans,
                app_lib::audio::speaker::run_assembly::OVERLAP_MAX_SPAN_SECS,
            );
            if !repair_candidates.is_empty() {
                let repair_spans: Vec<(f64, f64)> = repair_candidates
                    .iter()
                    .map(|c| (c.span.0 as f64 / 1000.0, c.span.1 as f64 / 1000.0))
                    .collect();
                for c in &repair_candidates {
                    eprintln!(
                        "CENSUS-REPAIR window=[{:.2}-{:.2}] donor={}",
                        c.span.0 as f64 / 1000.0,
                        c.span.1 as f64 / 1000.0,
                        c.donor_original_id
                    );
                }
                let model = std::env::var("MEETIFY_GATE_WHISPER_MODEL").unwrap_or_default();
                let lang = std::env::var("MEETIFY_GATE_LANG").unwrap_or_else(|_| "en".to_string());
                let embedding_path = format!(
                    "{models_dir}/{}",
                    app_lib::audio::speaker::model_download::embedding_filename()
                );
                let centroids_for_repair: Vec<(u32, Vec<f32>)> = out
                    .centroids
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect();
                let repair_samples = samples.clone();
                let repair_inputs = inputs.clone();
                let repair_refs = references.clone();
                // The mass decode closure consumed the first engine; the
                // repair decode loads its own instance (same store, same
                // model — the load happens on the async surface, the decode
                // inside spawn_blocking).
                let whisper_dir = std::path::Path::new(&home)
                    .join("AppData/Roaming/com.meetily.ai/models");
                let mut engine2 = app_lib::whisper_engine::WhisperEngine::new_with_models_dir(
                    Some(whisper_dir),
                )
                .expect("repair whisper engine");
                engine2.discover_models().await.expect("repair discover models");
                engine2.load_model(&model).await.expect("repair load model");
                let repairs = tokio::task::spawn_blocking(
                    move || -> Vec<app_lib::audio::speaker::run_assembly::SpanSynthesis> {
                        let Some(separator) =
                            app_lib::audio::speaker::separation::ConvTasNetSeparator::from_models_dir()
                        else {
                            return Vec::new();
                        };
                        let Ok(extractor2) =
                            NemoEmbeddingExtractor::new(&embedding_path)
                        else {
                            return Vec::new();
                        };
                        let embed = |audio: &[f32]| {
                            extractor2.extract_embedding(
                                audio,
                                app_lib::audio::speaker::run_engine::SAMPLE_RATE,
                            )
                        };
                        let (_, _, repair_inputs) =
                            app_lib::audio::speaker::run_engine::separated_stream_voice_votes(
                                &repair_samples,
                                &repair_inputs,
                                &embed,
                                &centroids_for_repair,
                                &repair_refs,
                                &separator,
                                &repair_spans,
                            );
                        let mut out = Vec::new();
                        for input in repair_inputs {
                            let [Some((c0, m0)), Some((c1, m1))] = input.identity else {
                                continue;
                            };
                            let decode = |s: &[f32]| -> Option<String> {
                                let text = engine2.transcribe_span_blocking(s.to_vec(), &lang)?;
                                let trimmed = text.trim();
                                if !trimmed.chars().any(|c| c.is_alphanumeric()) {
                                    return None;
                                }
                                let end_ms =
                                    s.len() as f64 / app_lib::audio::speaker::run_engine::SAMPLE_RATE as f64 * 1000.0;
                                let report = app_lib::audio::hallucination::audit(trimmed, 0.0, end_ms);
                                if report.is_garbage {
                                    None
                                } else {
                                    Some(trimmed.to_string())
                                }
                            };
                            let t0 = decode(&input.streams[0]);
                            let t1 = decode(&input.streams[1]);
                            out.push(app_lib::audio::speaker::run_assembly::SpanSynthesis {
                                span: input.span,
                                streams: [
                                    app_lib::audio::speaker::run_assembly::StreamEvidence {
                                        cluster: c0,
                                        margin: m0,
                                        rms_ratio: input.rms_ratio[0],
                                        text: t0,
                                    },
                                    app_lib::audio::speaker::run_assembly::StreamEvidence {
                                        cluster: c1,
                                        margin: m1,
                                        rms_ratio: input.rms_ratio[1],
                                        text: t1,
                                    },
                                ],
                                covered_atoms: input.covered_atoms,
                            });
                        }
                        out
                    },
                )
                .await
                .expect("repair decode join");
                let cluster_speaker = |c: u32| -> Option<String> {
                    Some(format!("Speaker {c}"))
                };
                let (next, applied) =
                    app_lib::audio::speaker::run_assembly::apply_loop_repairs(merged, &repairs, &cluster_speaker);
                eprintln!(
                    "GATE: phrase-loop repair applied {applied}/{} window(s)",
                    repairs.len()
                );
                merged = next;
            }
        }
        // Token-only census over every candidate span (walls, identities,
        // margins, RMS ratios, word counts, text sha256) — the ear-
        // calibration inventory. Verbatim stream text prints ONLY under
        // MEETIFY_RENDER_PRINT (terminal, never recorded).
        for cand in &gate_synthesis {
            use sha2::{Digest, Sha256};
            let sha = |t: &str| format!("{:x}", Sha256::digest(t.as_bytes()));
            let words = |t: &str| t.split_whitespace().count();
            let (w0, w1) = (
                cand.streams[0].text.as_deref().map(words).unwrap_or(0),
                cand.streams[1].text.as_deref().map(words).unwrap_or(0),
            );
            let fired = out
                .overlap_spans
                .iter()
                .any(|&(a, b)| a <= cand.span.0 && cand.span.1 <= b);
            let ids: Vec<Option<u32>> = cand
                .streams
                .iter()
                .map(|st| if st.text.is_some() { Some(st.cluster) } else { None })
                .collect();
            eprintln!(
                "CENSUS-SYN span=[{:.2}-{:.2}] span_in_spans={} ids=({:?},{:?}) margins=({:.3},{:.3}) rms=({:.3},{:.3}) words=({}, {}) sha=({},{}) verdict={}",
                cand.span.0,
                cand.span.1,
                fired,
                ids[0],
                ids[1],
                cand.streams[0].margin,
                cand.streams[1].margin,
                cand.streams[0].rms_ratio,
                cand.streams[1].rms_ratio,
                w0,
                w1,
                sha(cand.streams[0].text.as_deref().unwrap_or("")),
                sha(cand.streams[1].text.as_deref().unwrap_or("")),
                if ids[0].is_some() && ids[1].is_some() && ids[0] != ids[1] { "SYNTH" } else { "DEGRADE" },
            );
            if std::env::var("MEETIFY_RENDER_PRINT").is_ok() {
                eprintln!(
                    "CENSUS-TEXT [{:.2}-{:.2}] A={:?} B={:?}",
                    cand.span.0, cand.span.1, cand.streams[0].text, cand.streams[1].text
                );
            }
        }
        eprintln!(
            "GATE: synthesis replay: {} candidate span(s), {} synthesized, {} manual stand-down span(s)",
            gate_synthesis.len(),
            gate_synthesized,
            gate_manual_spans.len()
        );
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
        // Provenance parallel to refs (thread A): a group's identity is the
        // id set of its absorbed rows; two overlapping-wall groups sharing a
        // source id are ATTESTED simultaneous speech (the both-streams gate
        // is the attestation), never double-decode suspects.
        let row_source_ids: Vec<&str> = merged.iter().map(|r| r.original_id.as_str()).collect();
        let row_is_synth: Vec<bool> = merged.iter().map(|r| r.synth_atom).collect();
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
        // RENDER-TEXT acceptance (task 4.3, re-pinned to the current decode
        // after ear verdict 2026-09-20): the user reads PERSISTED TEXT, not
        // spans — proportional wall placement shifts ~1 s across long rows,
        // so a span window can pass vacuously while the "Oh, man." words
        // render under UserA (the exact S2b regression the user caught).
        // Anchor on the TEXT: every fragment containing "Oh, man" must carry
        // UserB's cluster (1 = UserB in this fixture's cluster order;
        // 0 = UserA speaks first at 1.11 s).
        {
            let bad: Vec<&app_lib::audio::speaker::alignment::AlignedSegment> = merged
                .iter()
                .filter(|s| {
                    let norm = s.text.to_lowercase();
                    norm.contains("oh, man") || norm.contains("oh man")
                })
                .filter(|s| s.speaker != "Speaker 1")
                .collect();
            let total: Vec<&app_lib::audio::speaker::alignment::AlignedSegment> = merged
                .iter()
                .filter(|s| {
                    let norm = s.text.to_lowercase();
                    norm.contains("oh, man") || norm.contains("oh man")
                })
                .collect();
            let ok = !total.is_empty() && bad.is_empty();
            eprintln!(
                "RENDER-TEXT 'Oh, man' -> {} fragment(s), labels {:?}: {}",
                total.len(),
                total.iter().map(|s| s.speaker.as_str()).collect::<Vec<_>>(),
                if ok { "OK" } else { "MISMATCH" }
            );
            if !ok {
                render_failures.push(format!(
                    "render-text acceptance failed: 'Oh, man' fragment(s) not under UserB (Speaker 1): labels {:?}",
                    total.iter().map(|s| s.speaker.as_str()).collect::<Vec<_>>()
                ));
            }
        }
        // RENDER-TEXT acceptance (ear verdicts 2026-09-22: people don't
        // monologue). (a) The response to "I have some updates" is another
        // voice than UserB's — ear + TitaNet agree it's UserA (Speaker
        // 0). (b) The recording question and its "Yeah, sure, sure" answer
        // are TWO DIFFERENT voices — the user attested the structure (the
        // asker is not the answerer); the specific mapping is NOT attested,
        // so only difference is pinned.
        {
            // Scoped to the attested spot: the response following
            // "I have some updates" in the opening exchange (before 30 s —
            // UserB says "roadmap" legitimately later in the meeting).
            let total: Vec<&app_lib::audio::speaker::alignment::AlignedSegment> = merged
                .iter()
                .filter(|s| {
                    s.text.to_lowercase().contains("on the roadmap")
                        && s.audio_start_ms < 30_000
                })
                .collect();
            let bad: Vec<
                &app_lib::audio::speaker::alignment::AlignedSegment,
            > = total.iter().copied().filter(|s| s.speaker != "Speaker 0").collect();
            let ok = !total.is_empty() && bad.is_empty();
            eprintln!(
                "RENDER-TEXT 'on the roadmap' -> {} fragment(s), labels {:?}: {}",
                total.len(),
                total.iter().map(|s| s.speaker.as_str()).collect::<Vec<_>>(),
                if ok { "OK" } else { "MISMATCH" }
            );
            if !ok {
                render_failures.push(
                    "render-text acceptance failed: response to updates not under UserA (Speaker 0)"
                        .to_string(),
                );
            }
        }
        // RENDER-TEXT acceptance (ear verdict 2026-09-06 + TitaNet wall
        // probe 2026-09-24 agree): "Gotcha." between UserB's "Yeah." and
        // her "Where is UserC?" is USERA (Speaker 0). At its true token
        // walls the clip scores UserA at margin 0.46; the wall-atom votes
        // must carry it out of UserB's fused row.
        {
            let total: Vec<&app_lib::audio::speaker::alignment::AlignedSegment> = merged
                .iter()
                .filter(|s| {
                    s.text.to_lowercase().contains("gotcha") && s.audio_start_ms < 40_000
                })
                .collect();
            let bad: Vec<
                &app_lib::audio::speaker::alignment::AlignedSegment,
            > = total.iter().copied().filter(|s| s.speaker != "Speaker 0").collect();
            let ok = !total.is_empty() && bad.is_empty();
            eprintln!(
                "RENDER-TEXT 'Gotcha' -> {} fragment(s), labels {:?}: {}",
                total.len(),
                total.iter().map(|s| s.speaker.as_str()).collect::<Vec<_>>(),
                if ok { "OK" } else { "MISMATCH" }
            );
            if !ok {
                render_failures.push(
                    "render-text acceptance failed: 'Gotcha' not under UserA (Speaker 0)"
                        .to_string(),
                );
            }
        }
        // RENDER-TEXT acceptance (ear ruling 2026-09-24, task 4.4 resolved):
        // "Oh, you're wearing the t-shirt" inside the 45.19-55.12 row is
        // USERA's — TitaNet's sustained-split vote there was wrong and the
        // render must keep the row whole under Speaker 0.
        {
            let total: Vec<&app_lib::audio::speaker::alignment::AlignedSegment> = merged
                .iter()
                .filter(|s| {
                    s.text.to_lowercase().contains("wearing the t-shirt")
                        && s.audio_start_ms > 44_000
                        && s.audio_start_ms < 56_000
                })
                .collect();
            let bad: Vec<
                &app_lib::audio::speaker::alignment::AlignedSegment,
            > = total.iter().copied().filter(|s| s.speaker != "Speaker 0").collect();
            let ok = !total.is_empty() && bad.is_empty();
            eprintln!(
                "RENDER-TEXT 't-shirt' -> {} fragment(s), labels {:?}: {}",
                total.len(),
                total.iter().map(|s| s.speaker.as_str()).collect::<Vec<_>>(),
                if ok { "OK" } else { "MISMATCH" }
            );
            if !ok {
                render_failures.push(
                    "render-text acceptance failed: 'wearing the t-shirt' not under UserA (Speaker 0)"
                        .to_string(),
                );
            }
        }
        // RENDER-TEXT acceptance (ear ruling 2026-09-24, INVARIANT; HARD pin
        // since turn-boundary-wall-realignment 1.4): "I don't know" is
        // USERA's, re-anchored past the 36.08 voice boundary (user replay
        // 2026-09-25: words at 36.1-36.7; whisper's walls were DTW-smeared).
        {
            let total: Vec<&app_lib::audio::speaker::alignment::AlignedSegment> = merged
                .iter()
                .filter(|s| {
                    s.text.to_lowercase().contains("i don't know")
                        && s.audio_start_ms >= 35_800
                        && s.audio_start_ms <= 36_400
                })
                .collect();
            let bad: Vec<
                &app_lib::audio::speaker::alignment::AlignedSegment,
            > = total.iter().copied().filter(|s| s.speaker != "Speaker 0").collect();
            let ok = !total.is_empty() && bad.is_empty();
            eprintln!(
                "RENDER-TEXT 'I don't know' -> {} fragment(s), labels {:?}: {}",
                total.len(),
                total.iter().map(|s| s.speaker.as_str()).collect::<Vec<_>>(),
                if ok { "OK" } else { "MISMATCH" }
            );
            if !ok {
                render_failures.push(format!(
                    "S7c_idontknow_userA: 'I don't know' fragment(s) under {:?} — ear ruling says UserA (Speaker 0)",
                    total.iter().map(|s| s.speaker.as_str()).collect::<Vec<_>>()
                ));
            }
        }
        {
            let q: Vec<&app_lib::audio::speaker::alignment::AlignedSegment> = merged
                .iter()
                .filter(|s| s.text.to_lowercase().contains("record this"))
                .collect();
            let a: Vec<&app_lib::audio::speaker::alignment::AlignedSegment> = merged
                .iter()
                .filter(|s| s.text.to_lowercase().contains("sure, sure")
                    || s.text.to_lowercase().contains("sure sure"))
                .collect();
            let q_badges: Vec<&str> = q.iter().map(|s| s.speaker.as_str()).collect();
            let a_badges: Vec<&str> = a.iter().map(|s| s.speaker.as_str()).collect();
            let ok = !q.is_empty() && !a.is_empty() && q_badges != a_badges;
            eprintln!(
                "RENDER-TEXT Q/A split: question labels {:?}, answer labels {:?}: {}",
                q_badges,
                a_badges,
                if ok { "OK" } else { "MISMATCH" }
            );
            if !ok {
                render_failures.push(
                    "render-text acceptance failed: recording question and its answer share one badge (self-answer monologue)"
                        .to_string(),
                );
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
            // Provenance-scoped exemption (thread A): two synthesized stream
            // rows sharing one source id are attested SIMULTANEOUS speech —
            // their cross-badge adjacency is the attested phenomenon, not a
            // sentence cut. Rows with disjoint source ids still fracture.
            let attested_pair = |a: usize, b: usize| -> bool {
                row_is_synth[a]
                    && row_is_synth[b]
                    && row_source_ids[a] == row_source_ids[b]
            };
            let prev_is_synth_pair = prev
                .row_indexes
                .iter()
                .any(|&a| {
                    g.row_indexes.iter().any(|&b| attested_pair(a, b))
                });
            if !prev_is_synth_pair
                && g.turn.speaker != prev.turn.speaker
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

        // overlap-stream-retranscription 3.3: the attested S16 crosstalk
        // window must render two distinct-badge rows carrying the attested
        // per-voice needles (verbatim in THIS local fixture, never code).
        // While S16 is a KNOWN-LIMITATION this routes through the amendment
        // path (AMENDED, not a failure); graduation (task 4.2) makes it a
        // hard pin by removing the waiver.
        {
            let s16 = fixture
                .entries
                .iter()
                .find(|e| e.id == "S16_ads_overlap_1056")
                .expect("S16 entry present");
            let window_rows: Vec<&app_lib::audio::speaker::turns::SpeakerTurn> = groups
                .iter()
                .map(|g| &g.turn)
                .filter(|t| t.start_ms < (s16.end_s * 1000.0) as i64 && ((s16.start_s * 1000.0) as i64) < t.end_ms)
                .collect();
            let badges: std::collections::BTreeSet<&str> =
                window_rows.iter().map(|t| t.speaker.as_str()).collect();
            let lowered: Vec<String> = window_rows
                .iter()
                .map(|t| t.text.to_lowercase())
                .collect();
            let needles: Vec<&String> = s16
                .needles
                .iter()
                .filter(|n| lowered.iter().any(|t| t.contains(&n.to_lowercase())))
                .collect();
            let ok = badges.len() >= 2 && needles.len() == s16.needles.len();
            eprintln!(
                "GATE: S16 stream render: {} row(s), {} distinct badge(s), {}/{} needle(s)",
                window_rows.len(),
                badges.len(),
                needles.len(),
                s16.needles.len()
            );
            if !ok {
                // Waiver state lives on the ATTRIBUTION fixture (this is the
                // entry's own amendment; the render snapshot's list is a
                // different document). While waived: AMENDED, not a failure —
                // graduation (4.2) removes the waiver and this becomes a
                // hard pin.
                let waived = fixture.known_limitations.iter().any(|k| k == "S16_ads_overlap_1056")
                    && fixture
                        .amendments
                        .get("S16_ads_overlap_1056")
                        .map_or(false, |a| !a.user_confirmed.is_empty() && !a.reason.is_empty());
                let msg = format!(
                    "S16 window renders {} distinct badge(s), {}/{} stream needle(s) — per-voice render missing or wrong",
                    badges.len(),
                    needles.len(),
                    s16.needles.len()
                );
                if waived {
                    eprintln!("AMENDED S16_ads_overlap_1056 — stream render not yet per-voice | {msg}");
                } else {
                    render_failures.push(format!("S16 stream render: {msg}"));
                }
            }
        }

        // overlap-stream-retranscription: the S18 fabrication window. The
        // phrase-loop repair MUST fire here — a degrade re-renders the
        // fabricated mixture rows, which is exactly the failure the user's
        // ear round demanded fixed. No waiver path: the fabrication is a
        // hard absence, and the attested words are a hard presence.
        {
            let s18 = fixture
                .entries
                .iter()
                .find(|e| e.id == "S18_clip01_talkover_192s")
                .expect("S18 entry present");
            let window_rows: Vec<&app_lib::audio::speaker::turns::SpeakerTurn> = groups
                .iter()
                .map(|g| &g.turn)
                .filter(|t| {
                    t.start_ms < (s18.end_s * 1000.0) as i64
                        && ((s18.start_s * 1000.0) as i64) < t.end_ms
                })
                .collect();
            let lowered: Vec<String> =
                window_rows.iter().map(|t| t.text.to_lowercase()).collect();
            // The attested truth is about the REGION's text, not row walls:
            // match against the joined window text so a needle straddling a
            // row boundary (the consolidation's chop points are not pinned)
            // still counts.
            let joined = lowered.join(" ");
            let fabrication: Vec<&String> = s18
                .absence_needles
                .iter()
                .filter(|n| joined.contains(&n.to_lowercase()))
                .collect();
            if !fabrication.is_empty() {
                render_failures.push(format!(
                    "S18 window still renders the fabricated phrase {fabrication:?} — phrase-loop repair did not fire or degraded"
                ));
            }
            let present: Vec<&String> = s18
                .needles
                .iter()
                .filter(|n| joined.contains(&n.to_lowercase()))
                .collect();
            let missing: Vec<&String> = s18
                .needles
                .iter()
                .filter(|n| !joined.contains(&n.to_lowercase()))
                .collect();
            eprintln!(
                "GATE: S18 render: fabrication {} ({}/{} absence), {}/{} attested needle(s), {} row(s)",
                if fabrication.is_empty() { "absent" } else { "PRESENT" },
                s18.absence_needles.len() - fabrication.len(),
                s18.absence_needles.len(),
                present.len(),
                s18.needles.len(),
                window_rows.len()
            );
            if !missing.is_empty() {
                eprintln!("GATE: S18 missing attested needle(s): {missing:?}");
            }
            if present.len() != s18.needles.len() {
                render_failures.push(format!(
                    "S18 window holds {}/{} attested needle(s) — the repair's per-voice render is missing attested words",
                    present.len(),
                    s18.needles.len()
                ));
            }
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
                // reported and failed, NEVER dropped silently here — unless
                // the pair is attested simultaneous (synth rows sharing one
                // source id: the both-streams gate attested them).
                let gi = groups[i]
                    .row_indexes
                    .iter()
                    .flat_map(|&x| groups[j].row_indexes.iter().map(move |&y| (x, y)))
                    .any(|(x, y)| {
                        row_is_synth[x]
                            && row_is_synth[y]
                            && row_source_ids[x] == row_source_ids[y]
                    });
                if !gi && a.start_ms < b.end_ms && b.start_ms < a.end_ms {
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

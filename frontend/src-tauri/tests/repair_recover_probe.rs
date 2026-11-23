//! Lever-ladder repair-recover probe (talkover-word-order-repair task 2.4):
//! for each kept window shape (the recorded shapes of the archived
//! `overlap-repair-net-worse-guard` offline-evidence.md) drive the FULL
//! builder+splice path offline — the live attested-window builder over the
//! meeting's persisted rows → production separation → span-level identity
//! (titanet refs, the census-validated read) → BOTH decode legs through the
//! shared utterance decode (leg A = existing strict profile; leg B = the
//! §2 no-condition repair profile) → the FROZEN selection rule → the live
//! splice — and evaluate the shape's ear-truth fixture pins on the selected
//! render.
//!
//! FROZEN comparison operator (recorded before measurement): the ear gate's
//! OWN pin semantics (`norm_tokens` = detokenize → lowercase → non-
//! alphanumeric → space, contiguous non-overlapping run counting, joined
//! scope over rows overlapping the pin window, `got != pin.count` fails).
//! The fixture pins are needles at attested multiplicity, not full
//! sequences — the design's "exact token-sequence equality" wording is
//! amended at the tick.
//!
//! FROZEN selection rule: `select_repair_pass` (pass 1 stands unless it
//! tripped a decode-quality guard AND the alternate pass cleared every
//! rejection guard).
//!
//! Two levels are recorded per shape: the SHIPPED-PATH outcome (attested
//! builder over the persisted rows, with the synth-region refusal the
//! runtime fold applies) and the RECOVERY MEASUREMENT (the splice layer
//! driven directly, synth flags stripped, so decode recovery is measured
//! independently of the current render's region state).
//!
//! Token-only record: walls, counts, ratios, margins, outcome tokens,
//! per-pin wanted/got counts. Needle tokens never enter the record;
//! verbatim mismatch diagnostics print ONLY under `MEETIFY_RENDER_PRINT`
//! (terminal only — nothing meeting-derived reaches the repo).
//!
//! Env:
//!   MEETIFY_LIVE_DIAG=1          (gate for any model load)
//!   MEETIFY_PROBE_SHAPES         (optional comma list of
//!                                 validated,validated-aligned,
//!                                 validated-recut,clean,control;
//!                                 default all)
//!   MEETIFY_MEETING_DIR          (recording dir name for the samples cache)
//!   MEETIFY_GATE_WHISPER_MODEL   (default large-v3-turbo-q5_0)
//!   MEETIFY_GATE_LANG            (default en)
//!   MEETIFY_RENDER_PRINT         (verbatim mismatch diagnostics, terminal)

#[path = "common/evidence_fixture.rs"]
mod evidence_fixture;

use app_lib::audio::speaker::alignment::{AlignedSegment, SpeakerSource};
use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use app_lib::audio::speaker::ports::{SeparatedStream, VoiceSeparationPort};
use app_lib::audio::speaker::run_assembly::{
    decode_stream_utterances, is_class_e_fiction, is_stuttering_decode, join_utterance_texts,
    repair_overlap_rows_checked, repair_windows_from_talkover_seeds, select_repair_pass,
    tail_guard_census_trace, tail_provenance_pass, LoopRepairCandidate, RepairPassEvidence,
    SelectedRepairPass, SpanSynthesis, StreamEvidence, TailGuardMode, TalkoverSeed,
    TalkoverSeedKind, OVERLAP_MAX_SPAN_SECS,
};
use app_lib::audio::speaker::turns::detokenize;
use app_lib::whisper_engine::whisper_engine::WhisperEngine;
use serde_json::Value;
use sha2::{Digest, Sha256};

const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";
const SR: usize = 16_000;
const EAR_FIXTURE: &str = "ear_truth_cde5c264.json";

// Recorded reference walls (overlap_census_probe.rs) — token-only
// coordinates into the samples cache.
const REF_WALLS: [(f64, f64); 3] = [(46.0, 50.0), (1196.2, 1199.5), (1201.0, 1210.0)];
const REF_NAMES: [&str; 3] = ["userA", "userB", "userC"];

// The three kept shapes — recorded coordinates (archived
// 2026-11-17-overlap-repair-net-worse-guard offline-evidence.md). The pin
// entry names the ear-truth fixture entry whose joined-scope pins are the
// shape's acceptance oracle (control is recorded only).
struct Shape {
    name: &'static str,
    window: (f64, f64),
    trigger: (f64, f64),
    pin_entry: Option<&'static str>,
}
const SHAPES: [Shape; 8] = [
    // The certified ship window in PRODUCTION carve coordinates: the entry
    // walls (1024609, 1057300) ms round-trip to these seconds, and the
    // production fold separates at THIS span — carving at the decimal
    // seconds (1024.61) would offset the stream audio by 16 samples (1ms)
    // and decode differently (2026-11-21: probe carve 08f55d47… vs
    // production bb7f9b19… — the certification must bind the carve
    // production actually makes).
    Shape {
        name: "validated",
        window: (1024609f64 / 1000.0, 1057300f64 / 1000.0),
        trigger: (1056.90, 1057.30),
        pin_entry: Some("S16_ads_overlap_1056"),
    },
    // The west-aligned re-cut of the certified window (2026-11-22): the
    // final production verification caught the splice absorbing the west
    // NULL-walled atom [1010.07,1024.748] WHOLE (any overlap absorbs the
    // whole atom), erasing 14.5s of out-of-window speech — the user's
    // mercenary-work sentence (addendum 8). This shape starts the window
    // at the real row boundary 1024.75: the west atom ends 1024.748, so
    // nothing west of the window is absorbed, no loss, no duplication.
    // East edge unchanged (the attested 1057.30); trigger still contained.
    // 1024.75 is exactly representable, so the ms round trip is exact.
    Shape {
        name: "validated-west-aligned",
        window: (1024750f64 / 1000.0, 1057300f64 / 1000.0),
        trigger: (1056.90, 1057.30),
        pin_entry: Some("S16_ads_overlap_1056"),
    },
    // Row-aligned re-cut of the S16 entry on the CURRENT render (the user's
    // 2026-11-18 ruling after the validated shape's wall-coverage STOP):
    // walls = the mega-row's own boundaries, so the splice absorbs whole
    // rows and erases nothing.
    Shape {
        name: "validated-aligned",
        window: (1042.40, 1074.42),
        trigger: (1056.90, 1057.30),
        pin_entry: Some("S16_ads_overlap_1056"),
    },
    // The wall-bearing-render re-cut (render-token-walls handoff, 2026-11-
    // 19): west edge row-aligned with the NULL-walled row's end (no absorb,
    // no erase), east edge token-aligned at 1056.50 on the walled mega-row
    // — occurrence #1 inside the window, occurrence #2 surviving east.
    // D4's owed decode re-measurement at these walls gates the S16 entry.
    // The trigger field records the REAL census trigger — which sits east
    // of the window; the runtime attested route seeds only on trigger
    // CONTAINMENT, so the entry's firing geometry is an open question the
    // record must carry, not hide (never hand-drawn).
    Shape {
        name: "validated-recut",
        window: (1024.75, 1056.50),
        trigger: (1056.90, 1057.30),
        pin_entry: Some("S16_ads_overlap_1056"),
    },
    // Search candidate 2 (2026-11-20, unattended window search): the
    // attested walls' west edge with the east edge at the first real
    // token boundary PAST the broken second occurrence — the wall
    // payload holds the second occurrence's tail as a degenerate
    // zero-duration cluster at 1057.43–1057.438, and candidate 1's
    // window [..,1057.30] split the mega-row before it. One extra real
    // token (1057.59-1057.76) rides inside for decode context; the
    // trigger stays contained.
    Shape {
        name: "attested-east",
        window: (1024.61, 1057.76),
        trigger: (1056.90, 1057.30),
        pin_entry: Some("S16_ads_overlap_1056"),
    },
    // Search candidate 3 (2026-11-20): east edge at the REAL boundary
    // 1056.90 — just before occurrence #2's "What" (1056.99). The west
    // piece's streams recover occurrence #1 (candidates 1-2 proved they
    // do); the east mixture piece KEEPS occurrence #2's text under its
    // real walls — the broken second occurrence stays mixture (its
    // degenerate zero-duration wall cluster mirrors the broken delivery
    // the user heard; a per-stream decode already failed to carry it in
    // candidates 1-2). Trigger-containment note: the census trigger
    // [1056.90,1057.30] STRADDLES this window's east edge, so the
    // runtime attested route (containment) would not seed it — a design
    // question recorded for the user, never hand-waved.
    Shape {
        name: "attested-west",
        window: (1024.61, 1056.90),
        trigger: (1056.90, 1057.30),
        pin_entry: Some("S16_ads_overlap_1056"),
    },
    Shape {
        name: "clean",
        window: (173.00, 203.00),
        trigger: (192.91, 193.76),
        pin_entry: Some("S18_clip01_talkover_192s"),
    },
    Shape {
        name: "control",
        window: (1045.30, 1057.30),
        trigger: (1056.90, 1057.30),
        pin_entry: None,
    },
];

/// One fixture pin: a needle at attested multiplicity. Tokens are read
/// locally from the evidence home and NEVER recorded or printed outside
/// the MEETIFY_RENDER_PRINT mismatch diagnostics (terminal only).
struct Pin {
    tokens: Vec<String>,
    count: usize,
}

struct Pins {
    window_s: (f64, f64),
    pins: Vec<Pin>,
}

/// Load one entry's joined-scope pins from the ear-truth fixture (local
/// evidence home only — nothing meeting-derived enters the repo).
fn load_pins(fixture: &Value, entry_name: &str) -> Pins {
    let entry = fixture
        .get("entries")
        .and_then(|e| e.as_array())
        .and_then(|entries| {
            entries
                .iter()
                .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(entry_name))
        })
        .unwrap_or_else(|| panic!("pin entry {entry_name} missing from the ear fixture"));
    let window_s = (
        entry.get("start_s").and_then(|v| v.as_f64()).expect("start_s"),
        entry.get("end_s").and_then(|v| v.as_f64()).expect("end_s"),
    );
    let empty: Vec<Value> = Vec::new();
    let pins = entry
        .get("pins")
        .and_then(|p| p.as_array())
        .unwrap_or(&empty)
        .iter()
        .map(|p| Pin {
            tokens: p
                .get("tokens")
                .and_then(|v| v.as_array())
                .expect("pin tokens")
                .iter()
                .map(|t| t.as_str().expect("token str").to_string())
                .collect(),
            count: p.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        })
        .collect();
    Pins { window_s, pins }
}

// FROZEN comparison operator — exact mirrors of the ear gate's own matcher
// (ear_truth_gate.rs `norm_tokens` / `count_token_run`; the gate is the
// comparison's source of truth, the probe never redefines it).

fn norm_tokens(text: &str) -> Vec<String> {
    detokenize(text)
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// Non-overlapping contiguous occurrence count of `run` in `haystack`
/// (ear_truth_gate.rs semantics).
fn count_token_run(haystack: &[String], run: &[String]) -> usize {
    if run.is_empty() || run.len() > haystack.len() {
        return 0;
    }
    let mut n = 0usize;
    let mut i = 0usize;
    while i + run.len() <= haystack.len() {
        if haystack[i..i + run.len()] == *run {
            n += 1;
            i += run.len();
        } else {
            i += 1;
        }
    }
    n
}

/// Joined-scope pin evaluation over the rows overlapping the pin window:
/// the gate's own joined semantics at row level (the runtime gate does the
/// turn-level check; the probe measures the render's token stream).
fn eval_pins(rows: &[AlignedSegment], pins: &Pins) -> Vec<(usize, usize, bool)> {
    let w0 = (pins.window_s.0 * 1000.0) as i64;
    let w1 = (pins.window_s.1 * 1000.0) as i64;
    let mut stream: Vec<String> = Vec::new();
    for r in rows
        .iter()
        .filter(|r| r.audio_start_ms < w1 && w0 < r.audio_end_ms)
    {
        stream.extend(norm_tokens(&r.text));
    }
    pins.pins
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let got = count_token_run(&stream, &p.tokens);
            (i, got, got == p.count)
        })
        .collect()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let dot: f32 = a[..n].iter().zip(&b[..n]).map(|(x, y)| x * y).sum();
    let na: f32 = a[..n].iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b[..n].iter().map(|x| x * x).sum::<f32>().sqrt();
    if na > 0.0 && nb > 0.0 {
        dot / (na * nb)
    } else {
        0.0
    }
}

/// The persisted rows overlapping `window` (read-only DB — the same
/// pattern the ear gate and the guard probe use).
async fn read_rows(window: (f64, f64)) -> Vec<(String, String, f64, f64, String, bool)> {
    let home = std::env::var("USERPROFILE").expect("USERPROFILE");
    let db =
        std::path::Path::new(&home).join("AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .read_only(true)
                .filename(&db),
        )
        .await
        .expect("open meeting DB read-only");
    let rows = sqlx::query_as::<_, (String, Option<String>, f64, f64, String, bool)>(
        "SELECT id, speaker_label, audio_start_time, audio_end_time, transcript, synth_atom \
         FROM transcripts WHERE meeting_id = ?1 AND audio_end_time > ?2 AND audio_start_time < ?3 \
         ORDER BY audio_start_time, audio_end_time",
    )
    .bind(MEETING_ID)
    .bind(window.0)
    .bind(window.1)
    .fetch_all(&pool)
    .await
    .expect("read persisted rows");
    pool.close().await;
    rows.into_iter()
        .map(|(id, b, s, e, t, synth)| {
            (id, b.unwrap_or_else(|| "Unknown".to_string()), s, e, t, synth)
        })
        .collect()
}

fn to_aligned(
    rows: &[(String, String, f64, f64, String, bool)],
    strip_synth: bool,
) -> Vec<AlignedSegment> {
    rows.iter()
        .map(|(id, badge, s, e, text, synth)| AlignedSegment {
            original_id: id.clone(),
            text: text.clone(),
            audio_start_ms: (*s * 1000.0) as i64,
            audio_end_ms: (*e * 1000.0) as i64,
            speaker: badge.clone(),
            speaker_source: SpeakerSource::Auto,
            // The splice layer never reads synth_atom; the builder does.
            // strip_synth drives the RECOVERY MEASUREMENT (region-refusal
            // semantics live at the shipped-path level, unit-pinned — see
            // module header).
            synth_atom: *synth && !strip_synth,
            synth_parent: None,
            token_walls: None,
        })
        .collect()
}

/// Production `StreamDecoder::decode` semantics (empty/punctuation-only →
/// abstain; hallucination-audit garbage → abstain) over one leg's decode
/// profile.
fn decode_leg(engine: &WhisperEngine, lang: &str, repair: bool, samples: &[f32]) -> Option<String> {
    let text = if repair {
        engine.transcribe_span_repair(samples.to_vec(), lang)?
    } else {
        engine.transcribe_span_blocking(samples.to_vec(), lang)?
    };
    let trimmed = text.trim();
    if !trimmed.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }
    let end_ms = samples.len() as f64 / SR as f64 * 1000.0;
    let report = app_lib::audio::hallucination::audit(trimmed, 0.0, end_ms);
    if report.is_garbage {
        return None;
    }
    Some(trimmed.to_string())
}

/// One decode pass over both separated streams (production utterance
/// decode: split at silences, per-utterance walls), assembled into the
/// splice's SpanSynthesis.
fn pass_synthesis(
    span: (f64, f64),
    streams: &[SeparatedStream],
    identity: &[(usize, f32); 2],
    decode: &dyn Fn(&[f32]) -> Option<String>,
) -> SpanSynthesis {
    let mut streams_iter = streams.iter();
    let mk = |i: usize, st: &SeparatedStream| {
        let utts = decode_stream_utterances(&st.samples, decode);
        StreamEvidence {
            cluster: identity[i].0 as u32,
            margin: identity[i].1,
            rms_ratio: st.pre_rms_ratio,
            text: join_utterance_texts(&utts),
            utterances: utts,
        }
    };
    let s0 = streams_iter.next().expect("stream 0");
    let s1 = streams_iter.next().expect("stream 1");
    SpanSynthesis {
        span,
        streams: [mk(0, s0), mk(1, s1)],
        covered_atoms: Vec::new(),
    }
}

/// Decode-quality guard evidence for the frozen selection rule: a pass
/// "tripped" when a stream abstained (audit-dropped), stuttered, or went
/// rate-fiction; it "cleared all guards" when its splice succeeded (every
/// rejection guard passed).
fn pass_evidence(synth: &SpanSynthesis, spliced: bool) -> RepairPassEvidence {
    let guard_tripped = synth.streams.iter().any(|st| {
        st.text
            .as_deref()
            .map(|t| is_stuttering_decode(t))
            .unwrap_or(true)
    }) || is_class_e_fiction(synth);
    RepairPassEvidence {
        guard_tripped,
        cleared_all_guards: spliced,
    }
}

/// Render facts for the record: how many rows the splice absorbed, how
/// many STRADDLE the window walls (their outside-window words leave the
/// render — the D7 absorb-whole consequence), and the erased-outside spans
/// per edge (token-only: seconds).
fn render_facts(rows: &[AlignedSegment], span_ms: (i64, i64)) -> (usize, usize, f64, f64) {
    let mut absorbed = 0usize;
    let mut straddling = 0usize;
    let mut west_erase = 0f64;
    let mut east_erase = 0f64;
    for r in rows {
        if r.audio_start_ms < span_ms.1 && span_ms.0 < r.audio_end_ms {
            absorbed += 1;
            if r.audio_start_ms < span_ms.0 {
                straddling += 1;
                west_erase += (span_ms.0 - r.audio_start_ms) as f64 / 1000.0;
            }
            if r.audio_end_ms > span_ms.1 {
                straddling += 1;
                east_erase += (r.audio_end_ms - span_ms.1) as f64 / 1000.0;
            }
        }
    }
    (absorbed, straddling, west_erase, east_erase)
}

#[tokio::test]
#[ignore = "live GPU probe: MEETIFY_LIVE_DIAG=1 cargo test --test repair_recover_probe -- --ignored --nocapture"]
async fn repair_recover_probe() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }

    // The pins are the acceptance oracle — no evidence home, no measurement
    // (loud skip; the helper prints the setup instructions).
    let Some(fixture_path) = evidence_fixture::evidence_fixture(EAR_FIXTURE) else {
        return;
    };
    let fixture: Value =
        serde_json::from_reader(std::fs::File::open(&fixture_path).expect("open ear fixture"))
            .expect("parse ear fixture");

    let print_verbatim = std::env::var("MEETIFY_RENDER_PRINT").is_ok();
    let model = std::env::var("MEETIFY_GATE_WHISPER_MODEL")
        .unwrap_or_else(|_| "large-v3-turbo-q5_0".to_string());
    let lang = std::env::var("MEETIFY_GATE_LANG").unwrap_or_else(|_| "en".to_string());

    let wanted: Option<Vec<String>> = std::env::var("MEETIFY_PROBE_SHAPES").ok().map(|s| {
        s.split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect()
    });

    // Models load once (async); the measurement loop runs inside
    // spawn_blocking (decodes are CPU/GPU-bound blocking work ONLY there).
    let models_dir = dirs::data_dir()
        .expect("no user data dir")
        .join("com.meetily.ai")
        .join("models");
    let mut engine =
        WhisperEngine::new_with_models_dir(Some(models_dir.clone())).expect("engine ctor");
    engine.discover_models().await.expect("discover models");
    engine.load_model(&model).await.expect("load model");

    // Per-shape DB reads happen on the async side; everything else moves
    // into the blocking closure.
    let mut shape_rows: Vec<Vec<(String, String, f64, f64, String, bool)>> = Vec::new();
    for shape in SHAPES.iter() {
        shape_rows.push(read_rows(shape.window).await);
    }

    tokio::task::spawn_blocking(move || {
        let separator =
            app_lib::audio::speaker::separation::ConvTasNetSeparator::from_models_dir()
                .expect("separator");
        // The embedding models live in ~/.meetily-models (the census
        // probe's root), not the runtime AppData dir (whisper's root).
        let home = std::env::var("USERPROFILE").expect("USERPROFILE");
        let embedding_models_dir = std::path::Path::new(&home).join(".meetily-models");
        let extractor = NemoEmbeddingExtractor::new(
            embedding_models_dir
                .join(app_lib::audio::speaker::model_download::embedding_filename())
                .to_str()
                .expect("models path utf-8"),
        )
        .expect("titanet extractor");

        // Samples cache (guard probe pattern) — read once for the refs.
        // MEETIFY_PROBE_AUDIO=production overrides to the PRODUCTION audio
        // provenance (decode the meeting's audio.mp4 through the app's own
        // decoder + sinc resample — the exact calls the Speakers run makes),
        // because the store entry's hash must bind the render production
        // actually splices, and the cache file is a snapshot of an earlier
        // decode (2026-11-21: cache-provenance hash 08f55d47… vs
        // production bb7f9b19… — the gate went stale-attestation, exactly
        // as designed).
        let meeting = std::env::var("MEETIFY_MEETING_DIR").expect("MEETIFY_MEETING_DIR");
        let home = std::env::var("USERPROFILE").expect("USERPROFILE");
        let music = std::path::Path::new(&home).join("Music");
        let production_audio = std::env::var("MEETIFY_PROBE_AUDIO")
            .map(|v| v == "production")
            .unwrap_or(false);
        let all: Vec<f32> = if production_audio {
            let mut audio = None;
            for entry in std::fs::read_dir(&music).expect("read Music") {
                let p = entry.expect("dir entry").path();
                if p.is_dir() && p.join(&meeting).join("audio.mp4").exists() {
                    audio = Some(p.join(&meeting).join("audio.mp4"));
                    break;
                }
            }
            let audio = audio.expect("audio.mp4 not found under Music");
            eprintln!("PROBE-AUDIO provenance=production path={}", audio.display());
            let decoded = app_lib::audio::decoder::decode_audio_file(&audio)
                .expect("production audio decode");
            decoded.to_whisper_format()
        } else {
            let mut cache = None;
            for entry in std::fs::read_dir(&music).expect("read Music") {
                let p = entry.expect("dir entry").path();
                if p.is_dir() && p.join(&meeting).join("samples_16k.f32").exists() {
                    cache = Some(p.join(&meeting).join("samples_16k.f32"));
                    break;
                }
            }
            let cache = cache.expect("samples_16k.f32 cache not found under Music");
            let raw = std::fs::read(&cache).expect("read samples cache");
            raw.chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()
        };

        // Reference embeddings (the census-validated identity geometry).
        let refs: Vec<Vec<f32>> = REF_WALLS
            .iter()
            .map(|(a, b)| {
                extractor
                    .extract_embedding(
                        &all[(*a * SR as f64) as usize..(*b * SR as f64) as usize],
                        SR as u32,
                    )
                    .expect("ref embedding")
            })
            .collect();

        let mut record: Vec<String> = Vec::new();
        let mut recovered = 0usize;
        let mut gated_shapes = 0usize;

        for (shape, rows) in SHAPES.iter().zip(&shape_rows) {
            if let Some(w) = &wanted {
                if !w.iter().any(|n| n == shape.name) {
                    continue;
                }
            }
            record.push(format!(
                "SHAPE name={} window=[{:.2},{:.2}] trigger=[{:.2},{:.2}] pins_entry={}",
                shape.name,
                shape.window.0,
                shape.window.1,
                shape.trigger.0,
                shape.trigger.1,
                shape.pin_entry.unwrap_or("-")
            ));

            let pins = shape.pin_entry.map(|e| load_pins(&fixture, e));
            let builder_rows = to_aligned(rows, false);
            let splice_rows = to_aligned(rows, true);
            record.push(format!(
                "DB rows={} synth_atoms={}",
                rows.len(),
                rows.iter().filter(|(_, _, _, _, _, s)| *s).count()
            ));

            // Baseline drift check: today's persisted render must hold the
            // pins (the attested render lives in the DB); a failure is a
            // coordinates/render drift finding, recorded before any decode.
            if let Some(p) = &pins {
                for (i, got, pass) in eval_pins(&builder_rows, p) {
                    record.push(format!(
                        "BASELINE-PIN pin{} want={} got={} {}",
                        i,
                        p.pins[i].count,
                        got,
                        if pass { "PASS" } else { "FAIL" }
                    ));
                }
            }

            // SHIPPED-PATH level: the attested builder over the persisted
            // rows (synth flags intact), then the runtime fold's
            // region-already-per-voice check.
            // Round, matching the gate's re-derivation (truncation loses 1ms
            // on round-tripped seconds — 1024609/1000.0 × 1000 → 1024608).
            let window_ms = (
                (shape.window.0 * 1000.0).round() as i64,
                (shape.window.1 * 1000.0).round() as i64,
            );
            let seeds = [TalkoverSeed {
                span: window_ms,
                kind: TalkoverSeedKind::Attested,
            }];
            let cands: Vec<LoopRepairCandidate> = repair_windows_from_talkover_seeds(
                &builder_rows,
                &seeds,
                &[],
                OVERLAP_MAX_SPAN_SECS,
            );
            match cands.first() {
                None => {
                    let synth_inside = builder_rows.iter().any(|r| {
                        r.synth_atom
                            && r.audio_start_ms < window_ms.1
                            && window_ms.0 < r.audio_end_ms
                    });
                    let why = if synth_inside {
                        "synth-row-inside"
                    } else {
                        "no-candidate"
                    };
                    record.push(format!("BUILDER candidates=0 refused={why}"));
                    record.push(format!("SHIPPED-PATH outcome=builder-refused:{why}"));
                }
                Some(c) => {
                    let walls_match = c.span == window_ms;
                    record.push(format!(
                        "BUILDER candidates=1 walls_match={} walls=[{},{}]",
                        walls_match, c.span.0, c.span.1
                    ));
                    let touches_synth = builder_rows.iter().any(|r| {
                        r.synth_atom && r.audio_start_ms < c.span.1 && c.span.0 < r.audio_end_ms
                    });
                    record.push(format!(
                        "SHIPPED-PATH outcome={}",
                        if touches_synth {
                            "region-already-per-voice"
                        } else {
                            "would-fire"
                        }
                    ));
                }
            }

            // RECOVERY MEASUREMENT: separation + identity + both legs +
            // frozen selection + splice (the splice layer driven directly;
            // region-refusal semantics live at the shipped-path level).
            let sep = separator.separate(&all, shape.window).expect("separation");
            if sep.len() != 2 {
                record.push(format!("SEPARATION streams={} ABORT", sep.len()));
                continue;
            }
            let mut identity: [(usize, f32); 2] = [(0, 0.0), (0, 0.0)];
            for (i, st) in sep.iter().enumerate() {
                let emb = extractor
                    .extract_embedding(&st.samples, SR as u32)
                    .expect("stream embedding");
                let mut scored: Vec<(f32, usize)> = refs
                    .iter()
                    .enumerate()
                    .map(|(j, r)| (cosine(r, &emb), j))
                    .collect();
                scored.sort_by(|a, b| b.0.total_cmp(&a.0));
                identity[i] = (scored[0].1, scored[0].0 - scored[1].0);
                record.push(format!(
                    "IDENTITY stream{i} pre_rms_ratio={:.4} top={} c1={:.4} c2={:.4} margin={:.4}",
                    st.pre_rms_ratio,
                    REF_NAMES[scored[0].1],
                    scored[0].0,
                    scored[1].0,
                    scored[0].0 - scored[1].0
                ));
            }

            // One pass per leg (synths built ONCE — no re-decode for the
            // selection evidence).
            let synth_a = pass_synthesis(shape.window, &sep, &identity, &|s| {
                decode_leg(&engine, &lang, false, s)
            });
            let synth_b = pass_synthesis(shape.window, &sep, &identity, &|s| {
                decode_leg(&engine, &lang, true, s)
            });
            // Tail-provenance guard pre-pass, census mode (wall-payload-
            // provenance-guard D1/D5): the SAME composition commands.rs
            // runs — the pre-pass sees each synthesis before the splice,
            // changes no bytes, and the census lines carry the design-D8
            // fields (counts, buckets, walls — token-only) for the
            // rule-selection measurement (interval-majority vs bare-point).
            let ref_speaker = |c: u32| REF_NAMES.get(c as usize).map(|s| s.to_string());
            for (leg_name, synth) in [("A-strict", &synth_a), ("B-repair", &synth_b)] {
                let (report, traces) = tail_guard_census_trace(synth, &splice_rows, &ref_speaker);
                record.push(format!(
                    "GUARD-CENSUS shape={} leg={leg_name} would_trim={} point_rule={}",
                    shape.name,
                    report.would_trims,
                    traces.iter().map(|t| t.trims_point_rule).sum::<usize>(),
                ));
                for t in &traces {
                    record.push(format!(
                        "GUARD-CENSUS-TRACE shape={} leg={leg_name} stream={} trims={} coverage={:.2} width_ms={} terminal_punct={} point_rule={}",
                        shape.name,
                        t.stream,
                        t.trims,
                        t.decisive_coverage_frac,
                        t.decisive_cover_width_ms,
                        t.terminal_punct_trim,
                        t.trims_point_rule,
                    ));
                }
            }
            // Placement-error distribution (design D8 a0): proportional
            // char-fraction walls vs REAL token walls on the persisted
            // payload rows — the empirical bound for the last-token
            // population the guard acts on.
            for r in &splice_rows {
                let Some(walls) = &r.token_walls else {
                    continue;
                };
                let total = r.text.chars().count().max(1);
                let dur = (r.audio_end_ms - r.audio_start_ms).max(1);
                let mut last_err = 0i64;
                let mut n = 0usize;
                let mut char_cursor = 0usize;
                for (i, tok) in r.text.split_whitespace().enumerate() {
                    let chars_before = char_cursor;
                    char_cursor += tok.chars().count() + 1;
                    let prop = r.audio_start_ms
                        + ((chars_before as f64 / total as f64) * dur as f64).round() as i64;
                    let err = walls
                        .get(i)
                        .map(|w| (prop - w.start_ms).abs())
                        .unwrap_or(0);
                    n += 1;
                    if i + 1 == r.text.split_whitespace().count() {
                        last_err = err;
                    }
                }
                record.push(format!(
                    "GUARD-PLACEMENT row=[{:.2},{:.2}] tokens={n} last_err_ms={last_err}",
                    r.audio_start_ms as f64 / 1000.0,
                    r.audio_end_ms as f64 / 1000.0,
                ));
            }
            let mut synth_a = synth_a;
            let mut synth_b = synth_b;
            tail_provenance_pass(&mut synth_a, &splice_rows, &ref_speaker, TailGuardMode::Census);
            tail_provenance_pass(&mut synth_b, &splice_rows, &ref_speaker, TailGuardMode::Census);
            let spliced_a =
                repair_overlap_rows_checked(&splice_rows, &synth_a, &|c: u32| {
                    REF_NAMES.get(c as usize).map(|s| s.to_string())
                });
            let spliced_b =
                repair_overlap_rows_checked(&splice_rows, &synth_b, &|c: u32| {
                    REF_NAMES.get(c as usize).map(|s| s.to_string())
                });

            for (leg_name, synth, spliced) in [
                ("A-strict", &synth_a, &spliced_a),
                ("B-repair", &synth_b, &spliced_b),
            ] {
                let token_counts: Vec<usize> = synth
                    .streams
                    .iter()
                    .map(|st| {
                        st.text
                            .as_deref()
                            .map(|t| norm_tokens(t).len())
                            .unwrap_or(0)
                    })
                    .collect();
                record.push(format!(
                    "PASS leg={leg_name} stream_tokens={token_counts:?} stutter={} fiction={} splice={}{}",
                    synth
                        .streams
                        .iter()
                        .any(|st| st.text.as_deref().map(is_stuttering_decode).unwrap_or(false)),
                    is_class_e_fiction(synth),
                    if spliced.is_ok() { "ok" } else { "rejected" },
                    match spliced {
                        Ok(_) => String::new(),
                        Err(e) => format!(":{e}"),
                    },
                ));
            }

            // FROZEN selection rule (run_assembly::select_repair_pass).
            let ev_a = pass_evidence(&synth_a, spliced_a.is_ok());
            let ev_b = pass_evidence(&synth_b, spliced_b.is_ok());
            let selected = select_repair_pass(&ev_a, Some(&ev_b));
            let leg_idx = match selected {
                SelectedRepairPass::First => 0usize,
                SelectedRepairPass::Alternate => 1usize,
            };
            let rule = if ev_a.guard_tripped
                && !ev_b.guard_tripped
                && ev_b.cleared_all_guards
            {
                "alternate-replaces-tripped-first"
            } else if ev_a.guard_tripped {
                "first-stands-alt-not-clearing"
            } else {
                "first-stands-first-clean"
            };
            record.push(format!(
                "SELECT leg={} rule={rule} first_tripped={} first_spliced={} alt_tripped={} alt_spliced={}",
                if leg_idx == 0 { "A-strict" } else { "B-repair" },
                ev_a.guard_tripped,
                ev_a.cleared_all_guards,
                ev_b.guard_tripped,
                ev_b.cleared_all_guards
            ));

            // The promoted render: the selected pass's splice result; a
            // failed splice stands down to the persisted rows (mixture).
            let spliced_sel = match leg_idx {
                0 => &spliced_a,
                _ => &spliced_b,
            };
            // The attestation-entry coordinates for this shape's selected
            // render (token-only, repo-safe): the content hash the runtime
            // splice gate recomputes (repair_render_sha256 semantics) and
            // the decode-profile fingerprint — exactly what an
            // attestedRepairWindows entry carries.
            let synth_sel = if leg_idx == 0 { &synth_a } else { &synth_b };
            record.push(format!(
                "RENDER-ATTESTATION-COORDS render_sha256={} profile_fp={}",
                app_lib::audio::speaker::run_assembly::repair_render_sha256(synth_sel),
                app_lib::audio::speaker::run_assembly::repair_profile_fp(&model),
            ));
            match spliced_sel {
                Ok(next) => {
                    let (absorbed, straddling, west_e, east_e) =
                        render_facts(&splice_rows, window_ms);
                    record.push(format!(
                        "RENDER spliced=yes rows={} absorbed={} straddling={} erase_west_s={:.2} erase_east_s={:.2}",
                        next.len(),
                        absorbed,
                        straddling,
                        west_e,
                        east_e
                    ));
                }
                Err(reason) => {
                    record.push(format!("RENDER spliced=no degraded=mixture reason={reason}"));
                }
            }
            let render: Vec<AlignedSegment> = match spliced_sel {
                Ok(next) => next.clone(),
                Err(_) => splice_rows.clone(),
            };

            // Acceptance: the shape's pins on the promoted render.
            let mut shape_ok = true;
            if let Some(p) = &pins {
                gated_shapes += 1;
                for (i, got, pass) in eval_pins(&render, p) {
                    record.push(format!(
                        "RENDER-PIN pin{} want={} got={} {}",
                        i,
                        p.pins[i].count,
                        got,
                        if pass { "PASS" } else { "FAIL" }
                    ));
                    shape_ok = shape_ok && pass;
                }
                // Verbatim row diagnostics — terminal only. Also dumped on a
                // PASS so the certification listen can show the winning rows.
                if print_verbatim {
                    let w0 = (p.window_s.0 * 1000.0) as i64;
                    let w1 = (p.window_s.1 * 1000.0) as i64;
                    for r in render
                        .iter()
                        .filter(|r| r.audio_start_ms < w1 && w0 < r.audio_end_ms)
                    {
                        eprintln!(
                            "RENDER-VERBATIM [{},{}] badge={}: {}",
                            r.audio_start_ms, r.audio_end_ms, r.speaker, r.text
                        );
                    }
                }
            }
            record.push(format!(
                "SHAPE-VERDICT name={} gated={} recovered={}",
                shape.name,
                pins.is_some(),
                if pins.is_none() {
                    "n/a-recorded-only"
                } else if shape_ok {
                    "YES"
                } else {
                    "NO"
                }
            ));
            if pins.is_some() && shape_ok {
                recovered += 1;
            }
        }

        for line in &record {
            eprintln!("{line}");
        }
        let body = record.join("\n");
        let sha = format!("{:x}", Sha256::digest(body.as_bytes()));
        eprintln!("RECORD-END sha256={sha}");
        eprintln!("SUMMARY shapes_gated={gated_shapes} recovered={recovered}/{gated_shapes}");
    })
    .await
    .expect("probe join");
}

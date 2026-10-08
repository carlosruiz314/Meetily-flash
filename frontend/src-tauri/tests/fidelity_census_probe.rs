//! Fidelity-scan calibration probe (diarization-render-fidelity 3.4).
//!
//! Loads the persisted render + immutable sources for the pinned meeting
//! from the local DB and drives the REAL scan fns (`scan_boundary_leaks`,
//! `scan_parent_links`) to measure near-miss rates BEFORE any gate
//! enforcement lands. Output is TOKEN-ONLY (ids, walls, chunk-length
//! histograms) — no meeting text ever prints.
//!
//! The dial decision: the boundary-token threshold (3) must separate the
//! synth-side histogram from the plain-side background. The full
//! shared-chunk histogram per population is the evidence.
//!
//! Run: MEETIFY_LIVE_DIAG=1 cargo test --test fidelity_census_probe -- --ignored --nocapture

#![cfg(test)]

use app_lib::audio::speaker::run_assembly::{scan_boundary_leaks, scan_parent_links, BOUNDARY_LEAK_MIN_CHUNK};
use sqlx::Row;

const DB_PATH: &str = "AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite";
const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";

fn home() -> String {
    std::env::var("USERPROFILE").unwrap()
}

/// Largest shared chunk at a pair's touching edges (tail vs head, both
/// directions) — the raw material for the dial histogram. Mirrors the
/// scan's edge pairing but computes BOTH populations (synth-side and
/// plain-side) so the background rate is visible.
fn boundary_pair_max_chunk(a_text: &str, b_text: &str, min_probe: usize) -> usize {
    let norm = |t: &str| -> Vec<String> {
        crate_norm(t)
    };
    let (a, b) = (norm(a_text), norm(b_text));
    let mut best = 0usize;
    let mut k = min_probe;
    while k <= a.len().min(b.len()) {
        if a[a.len() - k..] == b[..k] || a[..k] == b[b.len() - k..] {
            best = k;
        }
        k += 1;
    }
    best
}

// Keep the normalizer local: this probe must not re-implement the SCAN,
// only the histogram sweep around it. The scan itself is called for real.
fn crate_norm(t: &str) -> Vec<String> {
    t.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

#[tokio::test]
#[ignore = "local DB census: MEETIFY_LIVE_DIAG=1 cargo test --test fidelity_census_probe -- --ignored --nocapture"]
async fn fidelity_scan_calibration() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let db = format!("{}/{}", home(), DB_PATH);
    let pool = sqlx::sqlite::SqlitePool::connect(&format!("sqlite://{}", db.replace('\\', "/")))
        .await
        .expect("open local meeting DB");

    let rows = sqlx::query(
        "SELECT id, transcript, speaker, audio_start_time, audio_end_time, synth_atom, synth_parent \
         FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .expect("load render rows");

    let sources = sqlx::query(
        "SELECT id, audio_start_time, audio_end_time FROM transcript_sources WHERE meeting_id = ?",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .expect("load sources");

    pool.close().await;

    let get = |r: &sqlx::sqlite::SqliteRow, c: &str| r.try_get::<Option<f64>, _>(c).ok().flatten();
    let source_walls: Vec<(String, i64, i64)> = sources
        .iter()
        .filter_map(|r| {
            let s = get(r, "audio_start_time")?;
            let e = get(r, "audio_end_time")?;
            Some((r.get::<String, _>("id"), (s * 1000.0) as i64, (e * 1000.0) as i64))
        })
        .collect();

    // Rebuild AlignedSegments through the crate's public shape.
    let mut segs: Vec<app_lib::audio::speaker::alignment::AlignedSegment> = rows
        .iter()
        .filter_map(|r| {
            let s = get(r, "audio_start_time")?;
            let e = get(r, "audio_end_time")?;
            Some(app_lib::audio::speaker::alignment::AlignedSegment {
                original_id: r.get::<String, _>("id"),
                text: r.get::<String, _>("transcript"),
                audio_start_ms: (s * 1000.0) as i64,
                audio_end_ms: (e * 1000.0) as i64,
                speaker: r.get::<Option<String>, _>("speaker").unwrap_or_default(),
                speaker_source: app_lib::audio::speaker::alignment::SpeakerSource::Auto,
                synth_atom: r.get::<Option<i64>, _>("synth_atom").unwrap_or(0) != 0,
                synth_parent: r.get::<Option<String>, _>("synth_parent"),
            })
        })
        .collect();
    segs.sort_by_key(|s| (s.audio_start_ms, s.audio_end_ms));

    eprintln!(
        "PROBE: {} render rows, {} source rows, {} synth atoms",
        segs.len(),
        source_walls.len(),
        segs.iter().filter(|s| s.synth_atom).count()
    );

    // Dial histogram: largest shared boundary chunk per adjacent pair,
    // split by population (synth side present vs plain/plain).
    let mut synth_hist: std::collections::BTreeMap<usize, usize> = Default::default();
    let mut plain_hist: std::collections::BTreeMap<usize, usize> = Default::default();
    for w in segs.windows(2) {
        let shared = boundary_pair_max_chunk(&w[0].text, &w[1].text, 1);
        let hist = if w[0].synth_atom || w[1].synth_atom {
            &mut synth_hist
        } else {
            &mut plain_hist
        };
        *hist.entry(shared).or_insert(0) += 1;
    }
    eprintln!("PROBE: boundary shared-chunk histogram (chunk_len -> pair count)");
    let max_k = synth_hist.keys().chain(plain_hist.keys()).copied().max().unwrap_or(0);
    for k in 0..=max_k {
        eprintln!(
            "PROBE:   chunk={:>2}  synth-side={:<4} plain-side={:<4}",
            k,
            synth_hist.get(&k).copied().unwrap_or(0),
            plain_hist.get(&k).copied().unwrap_or(0)
        );
    }

    let leaks = scan_boundary_leaks(&segs, BOUNDARY_LEAK_MIN_CHUNK);
    eprintln!(
        "PROBE: scan_boundary_leaks(min_chunk={}) -> {} finding(s)",
        BOUNDARY_LEAK_MIN_CHUNK,
        leaks.len()
    );
    for f in &leaks {
        eprintln!(
            "PROBE:   LEAK synth_row={} synth=[{:.2},{:.2}] neighbour=[{:.2},{:.2}] dir={} chunk_len={}",
            f.synth_row_id,
            f.synth_span_ms.0 as f64 / 1000.0,
            f.synth_span_ms.1 as f64 / 1000.0,
            f.neighbour_span_ms.0 as f64 / 1000.0,
            f.neighbour_span_ms.1 as f64 / 1000.0,
            f.direction,
            f.chunk_len
        );
    }

    let parent_findings = scan_parent_links(&segs, &source_walls);
    let parent_links = segs.iter().filter(|s| s.synth_parent.is_some()).count();
    eprintln!(
        "PROBE: scan_parent_links -> {} finding(s) over {} parent link(s)",
        parent_findings.len(),
        parent_links
    );
    for f in &parent_findings {
        eprintln!(
            "PROBE:   PARENT row={} parent={} row=[{:.2},{:.2}] parent_span={} defect={:?}",
            f.row_id,
            f.parent_id,
            f.row_span_ms.0 as f64 / 1000.0,
            f.row_span_ms.1 as f64 / 1000.0,
            f.parent_span_ms
                .map(|(a, b)| format!("[{:.2},{:.2}]", a as f64 / 1000.0, b as f64 / 1000.0))
                .unwrap_or_else(|| "absent".to_string()),
            f.defect
        );
    }
}

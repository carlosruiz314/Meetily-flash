//! Ear-gate auditability for the repair-window attestations
//! (talkover-word-order-repair 4.1, design D2). TEST-side only: the
//! cross-check runs under `cargo test` via the evidence-home fixture
//! loader with its loud skip; production never reads any of this
//! (pinned structurally by `production_fixture_brightline.rs`).
//!
//! The two sides of the ledger:
//! - RUNTIME: the `attestedRepairWindows` allowlist in the local
//!   scheduler-settings store (token-only coordinates, never synced).
//! - FIXTURE: ear-truth entries of kind `repair_attestation` whose params
//!   equal the allowlist entry (the user's dated ear verdict made
//!   permanent).
//!
//! Negative cases use invented walls and synthetic pins — no real pin
//! content is transcribed into this file (bright line).

#[path = "common/evidence_fixture.rs"]
mod evidence_fixture;

use app_lib::audio::speaker::run_assembly::{
    attested_repair_windows_from_value, AttestedWindowEntry,
};
use serde_json::Value;

/// The fixture-side pin record: a `repair_attestation` entry's params.
#[derive(Debug, Clone, PartialEq)]
struct PinRecord {
    start_s: f64,
    end_s: f64,
    attested_date: String,
    render_sha256: String,
    profile_fp: String,
}

/// YYYY-MM-DD, strictly: two digits per field, real month/day ranges.
fn date_parses_iso(date: &str) -> bool {
    let b = date.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let (y, m, d) = (&date[0..4], &date[5..7], &date[8..10]);
    let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i32>(), m.parse::<u32>(), d.parse::<u32>()) else {
        return false;
    };
    (1..=9999).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d)
}

fn entry_walls_ms(e: &AttestedWindowEntry) -> (i64, i64) {
    ((e.start_s * 1000.0) as i64, (e.end_s * 1000.0) as i64)
}

/// Exact-wall, same-meeting pin match — the runtime gate's semantics
/// mirrored: overlapping-but-unequal and cross-meeting pins never match
/// (the runtime test pins that side; this keeps the lint from inventing a
/// looser match).
fn pin_for_entry<'a>(
    entry: &AttestedWindowEntry,
    meeting: &str,
    pins: &'a [PinRecord],
) -> Option<&'a PinRecord> {
    if entry.meeting_id != meeting {
        return None;
    }
    pins.iter().find(|p| {
        ((p.start_s * 1000.0) as i64, (p.end_s * 1000.0) as i64) == entry_walls_ms(entry)
    })
}

/// The fixture lint (talkover-word-order-repair 4.1): every finding names
/// its offending window — an empty list is the only pass.
fn lint_attestations(
    entries: &[AttestedWindowEntry],
    meeting: &str,
    pins: &[PinRecord],
) -> Vec<String> {
    let mut failures = Vec::new();
    // Entries unique by (meeting, walls) — duplicates fail.
    let mut seen: std::collections::BTreeMap<(String, i64, i64), usize> = Default::default();
    for e in entries {
        let key = (e.meeting_id.clone(), e.start_s as i64, e.end_s as i64);
        *seen.entry(key).or_insert(0) += 1;
    }
    for ((m, s, t), n) in &seen {
        if *n > 1 {
            failures.push(format!(
                "duplicate attestedRepairWindows entries for meeting {m} walls [{s},{t}]"
            ));
        }
    }
    // attested_date parses YYYY-MM-DD or the failure names the entry.
    for e in entries {
        if !date_parses_iso(&e.attested_date) {
            failures.push(format!(
                "entry for meeting {} walls [{},{}] has attested_date {:?} — not YYYY-MM-DD",
                e.meeting_id, e.start_s, e.end_s, e.attested_date
            ));
        }
    }
    // Every entry has an exact-wall, same-meeting fixture pin; a pin whose
    // hash or fingerprint differs from the entry's is stale and fails.
    for e in entries {
        match pin_for_entry(e, meeting, pins) {
            None => failures.push(format!(
                "attestedRepairWindows entry for meeting {} walls [{},{}] has no matching \
                 repair_attestation fixture pin",
                e.meeting_id, e.start_s, e.end_s
            )),
            Some(p) => {
                if p.render_sha256 != e.render_sha256 {
                    failures.push(format!(
                        "entry for meeting {} walls [{},{}] is stale: render_sha256 {} vs pin {}",
                        e.meeting_id,
                        e.start_s,
                        e.end_s,
                        &e.render_sha256[..8.min(e.render_sha256.len())],
                        &p.render_sha256[..8.min(p.render_sha256.len())]
                    ));
                }
                if p.profile_fp != e.profile_fp {
                    failures.push(format!(
                        "entry for meeting {} walls [{},{}] is stale: profile_fp mismatch",
                        e.meeting_id, e.start_s, e.end_s
                    ));
                }
            }
        }
    }
    failures
}

/// The reverse rule (delta: "Promoted per-voice rendered span without an
/// allowlist entry fails the gate"), scoped to PROMOTED spans via the
/// census record — the legacy per-voice path is NOT gated. Callers pass
/// the promoted spans the census recorded for the meeting.
fn lint_promoted_spans(
    promoted_spans_ms: &[(String, i64, i64)],
    entries: &[AttestedWindowEntry],
) -> Vec<String> {
    promoted_spans_ms
        .iter()
        .filter(|(meeting, s, t)| {
            !entries.iter().any(|e| {
                e.meeting_id == *meeting && entry_walls_ms(e) == (*s, *t)
            })
        })
        .map(|(meeting, s, t)| {
            format!(
                "promoted per-voice span for meeting {meeting} walls [{s},{t}] has no \
                 attestedRepairWindows entry"
            )
        })
        .collect()
}

fn synthetic_entry(meeting: &str, start_s: f64, end_s: f64) -> AttestedWindowEntry {
    AttestedWindowEntry {
        meeting_id: meeting.to_string(),
        start_s,
        end_s,
        attested_date: "2026-11-20".to_string(),
        render_sha256: "a".repeat(64),
        profile_fp: "fp".to_string(),
    }
}

fn synthetic_pin(start_s: f64, end_s: f64) -> PinRecord {
    PinRecord {
        start_s,
        end_s,
        attested_date: "2026-11-20".to_string(),
        render_sha256: "a".repeat(64),
        profile_fp: "fp".to_string(),
    }
}

// ---- pure lint (invented walls, synthetic pins) ----

#[test]
fn entry_without_exact_fixture_pin_fails() {
    let entries = vec![synthetic_entry("meet-1", 21.0, 24.0)];
    let pins = vec![synthetic_pin(21.0, 24.0)];
    assert!(lint_attestations(&entries, "meet-1", &pins).is_empty(), "exact match passes");

    // No pins at all.
    let failures = lint_attestations(&entries, "meet-1", &[]);
    assert_eq!(failures.len(), 1);
    assert!(failures[0].contains("[21,24]"), "names the window: {failures:?}");

    // Overlapping-but-unequal walls are NOT a match.
    let near_miss = vec![synthetic_pin(21.0, 25.0)];
    let failures = lint_attestations(&entries, "meet-1", &near_miss);
    assert_eq!(failures.len(), 1, "overlapping-but-unequal is unpinned: {failures:?}");

    // Cross-meeting pins never match (the pin lives in a fixture for ONE
    // meeting; the entry names its own).
    let failures = lint_attestations(&entries, "meet-2", &pins);
    assert_eq!(failures.len(), 1, "cross-meeting is unpinned: {failures:?}");
}

#[test]
fn stale_hash_or_fingerprint_fails_the_cross_check() {
    let entries = vec![synthetic_entry("meet-1", 21.0, 24.0)];
    let mut stale_sha = synthetic_pin(21.0, 24.0);
    stale_sha.render_sha256 = "b".repeat(64);
    let failures = lint_attestations(&entries, "meet-1", &[stale_sha]);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("stale"), "{failures:?}");

    let mut stale_fp = synthetic_pin(21.0, 24.0);
    stale_fp.profile_fp = "other-fp".to_string();
    let failures = lint_attestations(&entries, "meet-1", &[stale_fp]);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("profile_fp mismatch"), "{failures:?}");
}

#[test]
fn duplicate_entries_fail() {
    let entries = vec![
        synthetic_entry("meet-1", 21.0, 24.0),
        synthetic_entry("meet-1", 21.0, 24.0),
    ];
    let pins = vec![synthetic_pin(21.0, 24.0)];
    let failures = lint_attestations(&entries, "meet-1", &pins);
    assert!(
        failures.iter().any(|f| f.contains("duplicate")),
        "{failures:?}"
    );
}

#[test]
fn malformed_date_fails_naming_the_entry() {
    let mut e = synthetic_entry("meet-1", 21.0, 24.0);
    e.attested_date = "Nov 20 2026".to_string();
    let entries = vec![e];
    let pins = vec![synthetic_pin(21.0, 24.0)];
    let failures = lint_attestations(&entries, "meet-1", &pins);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("attested_date"), "{failures:?}");
    assert!(date_parses_iso("2026-11-20"));
    assert!(!date_parses_iso("2026-11-2"));
    assert!(!date_parses_iso("2026-13-01"));
    assert!(!date_parses_iso("20261120"));
}

#[test]
fn promoted_span_without_entry_fails_legacy_path_is_not_gated() {
    // The reverse rule, scoped to PROMOTED spans: a per-voice rendered
    // span the census recorded as a promoted rebuild needs its entry; the
    // legacy per-voice path never appears in that list, so it is
    // structurally ungated.
    let entries = vec![synthetic_entry("meet-1", 21.0, 24.0)];
    let promoted = vec![
        ("meet-1".to_string(), 21_000, 24_000), // covered
        ("meet-1".to_string(), 30_000, 32_000), // uncovered → fails
    ];
    let failures = lint_promoted_spans(&promoted, &entries);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("[30000,32000]"), "{failures:?}");
    assert!(lint_promoted_spans(&[("meet-1".to_string(), 21_000, 24_000)], &entries).is_empty());
}

// ---- live cross-check (loud skip when the evidence home is absent) ----

/// The local scheduler-settings store (the allowlist's home). Env override
/// `MEETIFY_SCHEDULER_STORE`; default is the standard per-OS app-config
/// location for the store file the scheduler settings use.
fn store_path() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("MEETIFY_SCHEDULER_STORE") {
        return Some(std::path::PathBuf::from(p));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return Some(
            std::path::PathBuf::from(appdata)
                .join("com.meetily.ai")
                .join("scheduler_settings.json"),
        );
    }
    std::env::var("HOME").ok().map(|home| {
        std::path::PathBuf::from(home)
            .join("Library/Application Support/com.meetily.ai/scheduler_settings.json")
    })
}

fn store_entries() -> Vec<AttestedWindowEntry> {
    let Some(path) = store_path() else {
        eprintln!("GATE: no store location resolvable — allowlist treated as empty");
        return Vec::new();
    };
    let Ok(body) = std::fs::read_to_string(&path) else {
        eprintln!(
            "GATE: store file {} not found — allowlist treated as empty (loud, not silent)",
            path.display()
        );
        return Vec::new();
    };
    let v: Value = serde_json::from_str(&body).expect("store file parses as JSON");
    let entries = attested_repair_windows_from_value(v.get("attestedRepairWindows"));
    eprintln!("GATE: allowlist carries {} entry(ies)", entries.len());
    entries
}

fn fixture_attestation_pins() -> Option<(String, Vec<PinRecord>)> {
    let path = evidence_fixture::evidence_fixture("ear_truth_cde5c264.json")?;
    let body = std::fs::read_to_string(path).expect("fixture reads");
    let v: Value = serde_json::from_str(&body).expect("fixture parses as JSON");
    let meeting = v
        .get("meeting")
        .and_then(|m| m.as_str())
        .expect("fixture names its meeting")
        .to_string();
    let pins = v["entries"]
        .as_array()
        .expect("fixture entries array")
        .iter()
        .filter(|e| e.get("kind").and_then(|k| k.as_str()) == Some("repair_attestation"))
        .map(|e| PinRecord {
            start_s: e["start_s"].as_f64().expect("pin start_s"),
            end_s: e["end_s"].as_f64().expect("pin end_s"),
            attested_date: e["attested_date"]
                .as_str()
                .expect("pin attested_date")
                .to_string(),
            render_sha256: e["render_sha256"]
                .as_str()
                .expect("pin render_sha256")
                .to_string(),
            profile_fp: e["profile_fp"].as_str().expect("pin profile_fp").to_string(),
        })
        .collect::<Vec<_>>();
    eprintln!(
        "GATE: fixture carries {} repair_attestation pin(s)",
        pins.len()
    );
    Some((meeting, pins))
}

#[test]
fn allowlist_cross_checks_the_fixture_ledger() {
    // Loud skip: the fixtures are the gate's only truth — a missing home
    // must never pass silently.
    let Some((meeting, pins)) = fixture_attestation_pins() else {
        return;
    };
    let entries = store_entries();
    let failures = lint_attestations(&entries, &meeting, &pins);
    assert!(
        failures.is_empty(),
        "attestation ledger out of sync — allowlist vs fixture:\n{}",
        failures.join("\n")
    );
    // The reverse rule over the live census record's promoted spans is
    // wired when §5.2/5.3 produce the first promoted span; the lint is
    // pinned above with synthetic spans.
}

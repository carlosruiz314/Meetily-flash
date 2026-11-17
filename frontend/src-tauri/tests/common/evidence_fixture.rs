//! Evidence-home fixture resolution (§6 of gap-speech-voice-attribution and
//! engine-boundary-and-identity-accuracy). The golden fixtures are
//! meeting-derived and live ONLY in the local evidence home — never in the
//! repo (bright line: nothing meeting-derived, even synthetic, is tracked).
//!
//! Resolution: `$MEETILY_LOCAL_EVIDENCE_DIR` → the repo's sibling
//! `../meetily-flash-local-evidence/` → `None`. Callers MUST skip or fail
//! LOUDLY on `None` — the fixtures are the gates' only truth, and a missing
//! home must never pass silently.

use std::path::PathBuf;

/// The evidence home's `tests-fixtures/` directory (env override respected).
pub fn evidence_dir() -> PathBuf {
    match std::env::var("MEETILY_LOCAL_EVIDENCE_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => PathBuf::from("../meetily-flash-local-evidence"),
    }
    .join("tests-fixtures")
}

/// Resolve a fixture by name under the evidence home's `tests-fixtures/`.
pub fn evidence_fixture(name: &str) -> Option<PathBuf> {
    let path = evidence_dir().join(name);
    if path.exists() {
        Some(path)
    } else {
        eprintln!(
            "EVIDENCE: fixture {} not found (searched {}); \
             set MEETILY_LOCAL_EVIDENCE_DIR or restore the evidence home \
             (see AGENTS.md §3) — skipping LOUDLY",
            name,
            path.display()
        );
        None
    }
}

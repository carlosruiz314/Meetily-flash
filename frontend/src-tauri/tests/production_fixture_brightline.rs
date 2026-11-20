//! Bright-line structural pin (talkover-word-order-repair 3.3, design
//! D2/D6): production code SHALL NOT read the evidence fixture — the
//! `attestedRepairWindows` allowlist is its only runtime surface. The
//! fixture loader (`evidence_fixture`) and its env name live ONLY under
//! `tests/`, which this scanner itself is part of (it reads `src/`, never
//! itself). Fails naming any `src/**/*.rs` file referencing the markers.

const MARKERS: [&str; 3] = [
    "evidence_fixture",
    "MEETIFY_LOCAL_EVIDENCE",
    "meetily-flash-local-evidence",
];

fn scan(dir: &std::path::Path, hits: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            scan(&path, hits);
        } else if path.extension().is_some_and(|x| x == "rs") {
            let Ok(body) = std::fs::read_to_string(&path) else {
                continue;
            };
            for marker in MARKERS {
                if body.contains(marker) {
                    hits.push(format!("{}: {marker}", path.display()));
                }
            }
        }
    }
}

#[test]
fn production_sources_never_reference_the_evidence_fixture() {
    let mut hits = Vec::new();
    scan(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut hits);
    assert!(
        hits.is_empty(),
        "production source references the evidence fixture: {hits:?}"
    );
}

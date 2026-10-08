# Census-replay fixtures (diarization-render-fidelity, design D6)

Two-tier fixture split — the PII bright line covers meeting-derived
SYNTHETIC data too:

1. **Repo-committed (this directory): PURELY SYNTHETIC.** Invented texts
   ("alpha beta gamma"), invented walls, engineered defect shapes that no
   real meeting span inspired. They prove the harness mechanics, pin the
   fake-decoder contracts, and run the property tests in plain
   `cargo test` (milliseconds, no GPU, no models, no audio).
2. **Meeting-shaped: `MEETILY_LOCAL_EVIDENCE_DIR/census_replay/*.json`.**
   The six mined spans' walls/word counts and recorded-shaped texts live
   there (the pre-push guard's protected home — that directory must never
   get a remote). The harness loads them when present and skips with a
   loud note when absent; the live ear-truth gate remains their parity
   pin.

Schema (`ReplayFixture` in `tests/census_replay_harness.rs`): rows, span,
covered atoms, identity pair, rms ratios, per-stream scripts (tone
windows for the synthetic audio + the scripted whole-stream decode text),
manual spans, label map, and the expected outcome (`degrade` =
byte-identical rows, or `replace` with pinned row shapes).

The fake-decoder contract is keyed by (span, stream) via per-call closure
scoping: the harness invokes the synthesis entry with ONE input per call
and a closure bound to that input's expected samples plus a call counter
— never a re-implemented decode loop.

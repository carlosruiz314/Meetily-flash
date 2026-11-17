# Rebuild notes — what survives of the dropped `overlap-scramble-repair`

The dropped change left NO recoverable commits (verified 2026-11-17: HEAD sat
at `2176e538` for the branch's whole life; the applied work was uncommitted
tree state discarded at the drop). These are the surviving fragments the
rebuild (tasks §3) starts from.

## Session-record API sketches (pure code names, no meeting content)

- `run_engine.rs`: `SpanDecodeEvidence { span, texts: [Option<String>; 2] }`;
  `scramble_seed_spans`; `scramble_evidence_spans` — the evidence pass
  produced per-span separated decodes; seeds were spans whose decode rate
  exceeded the pinned human speech ceiling (interleave fingerprint).
- `run_assembly.rs`: `stream_decode_rate_fiction` — the rate-fiction check
  the fingerprint repurposed as seed evidence.
- `commands.rs`: evidence pass + promotion wiring (the promoted window routed
  through the existing phrase-loop repair builder).
- `tests/ear_truth_gate.rs`: probe `scramble_repair_text_probe_1056` + WAV
  export + `MEETIFY_PROBE_WINDOW` env override (window-shape decode probe).

## Recorded measurements (fork issue #6 holds the full findings)

- Cold-gate census: 6.2 h meeting-wide evidence pass.
- Multiset containment mixture-vs-repair: 0.646 (validated window) vs 0.405
  (mangled window).
- Window shapes kept as clips: 32.69 s (validated), 30.68 s (mangled),
  12 s (interjection vanishes) — earcheck WAVs live in the meeting folder
  (evidence home holds the tokenized record).

## Where the surviving evidence lives

- `../meetily-flash-local-evidence/` — tokenized gate/ear-round record.
- The meeting folder (`~/Music/*-recordings/<meeting>/`) — earcheck-5/6
  mixture clips, earcheck-7 separated-stream WAVs for the three shapes.
- Fork issue #6 — the distilled findings and the blocker analysis.
- The live spec's repair machinery (windows, absorption, floors) — unchanged
  by the drop; the rebuild reuses it.

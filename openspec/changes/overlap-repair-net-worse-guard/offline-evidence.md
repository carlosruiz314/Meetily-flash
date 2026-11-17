# Offline guard-validation record (tasks 1.3–1.5)

Token-only record. The statistic was FROZEN before any number below was
recorded (task 1.4's ordering rule). Verbatim text never enters this file;
decodes print verbatim only to the terminal under `MEETIFY_RENDER_PRINT`.

## Frozen statistic (recorded 2026-11-17, before measurement)

- Alignment: per badge — each conservation group's ordered normalized
  tokens vs the stream whose vote resolves to that badge. Global
  (concatenated) alignment is rejected by test (interleaved fixture caps
  ≈ half).
- Tokens: whitespace-split, lowercased, punctuation stripped
  (alphanumeric-only), empties dropped — `normalize_tokens`.
- Recall (the bar's operand): LCS length / |conserved tokens| for the
  group. Repair-side tokens outside every group cost nothing.
- Precision (diagnostic, recorded not gated): LCS / |stream tokens|.
- Minimum-set handling: a group with < 3 conserved tokens is recorded but
  NOT gated (too small for a meaningful ratio); the empty conservation set
  refuses the promotion outright (guard rule, task 2.1).
- Badge resolution: the probe records the FULL group×stream matrix plus
  the per-group best assignment (the statistic's upper bound); the runtime
  guard uses vote-resolved badges (task 3.3), where a bad vote degrades
  recall and stands the promotion down (conservative).

## Measurement plan (coordinates from the fixture + earcheck-7 WAV shapes)

| Shape | Window (s) | Seed span (s) | Expected |
|---|---|---|---|
| validated | 1024.61–1057.30 (32.69s) | 1056.90–1057.30 (classE_clip08_1056s) | passes the bar |
| mangled | 173.00–203.68 (30.68s) | 192.91–193.76 (the talk-over span) | fails the bar |
| row-removed | 173.68–203.68 (30.00s) | 192.91–193.76 | restores the tail (higher recall) |
| control 12s | 1045.30–1057.30 (12.00s) | 1056.90–1057.30 | recorded, expected ~no-change |

Baseline: the meeting's own persisted rows overlapping the window (read-only
DB), scoped by `build_conservation_groups`.

## Reproduction criterion (task 1.4)

The re-derived validated render must score HIGH against its conservation
groups; the mangled shape must score LOW with the divergence localized to
the attested tail region (checked under `MEETIFY_RENDER_PRINT` against the
fixture's attested pin sequences). A mismatch is a recorded STOP — window
coordinates or model drift suspected, user rules.

## Results (measured 2026-11-17; token-only)

Scoping correction discovered by the reproduction criterion BEFORE any
verdict was recorded: conservation groups must clip to the WINDOW walls
first (the promotion replaces only within-window text; beyond-wall text
persists as outside pieces). Implemented + red-tested
(`rows_are_clipped_to_the_window_before_seed_scoping`), then measured.
The first (unclipped) run's numbers are superseded.

Best per-badge recall vs the meeting's persisted baseline
(group×stream matrix recorded in the probe logs; per-group best shown):

| Shape | Window (s) | conserved tokens (per badge) | best recalls | Render verdict |
|---|---|---|---|---|
| validated (S16) | 1024.61–1057.30 | 46 / 28 | 0.652 / 0.857 | user-validated 2026-11-16 |
| mangled (S18, straddled row IN) | ~172.32–203.00 (30.68s; recorded streams) | 44 / 31 | 0.818 / 0.903 | attested tail paraphrased (fluent substitution of 2–3 connective/noun tokens in the tail row) |
| clean (S18, row removed) | 173.00–203.00 (30.0s; recorded streams) | 42 / 30 | 0.857 / 1.000 | tail matches the attested sequence verbatim |
| 12s control | 1045.30–1057.30 | 29 | 0.586 | interjection absent (as attested); single-badge window |

Reproduction criterion: MET — the clean shape restores the attested tail
verbatim and the straddled-row shape paraphrases it, exactly as the issue
records ("removing the D7-straddled row restores the attested tail").
The harmful extra west context is the straddled row's ~0.68s.

## Stop-point verdict (task 1.5): the evidence cannot separate the cases — NO bar exists

- The mangled shape scores HIGHER (0.818 / 0.903) than the user-validated
  shape (0.652 / 0.857). Any bar low enough to accept the validated render
  accepts the mangle; any bar high enough to reject the mangle rejects the
  validated render.
- Mechanism (measured): the mangle costs only 2–3 substituted tokens in a
  ~30-token tail row (recall −0.10 at most), while the validated render's
  separation word-loss and cross-bleed cost far more tokens — the guard's
  noise (separation word drop, cross-stream bleed: a voice's tokens land in
  BOTH streams; the non-assigned stream scores 0.18–0.37) dwarfs the signal
  (fluent substitution).
- The issue's window-level multiset numbers (0.646 vs 0.405) compared the
  repair against a MIXTURE decode of the same window; against the persisted
  baseline the separation inverts. The remaining untested runtime baseline
  (mixture decode of the window, one extra Whisper call) was already
  dismissed by the issue as order-blind and n=1 dial-fitting, and the
  paraphrase's tokens largely exist in the mixture jumble, so containment
  cannot see this class either.
- Conclusion: a token-conservation net-worse guard cannot ship. The
  paraphrase defect is a meaning-level defect; the only oracle that rules
  it is the ear round (or the fixture pins — forbidden in code paths by the
  bright line). Escalated to the user per task 1.5's mandatory ruling.

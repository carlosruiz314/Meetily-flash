# Tasks: overlap-stream-retranscription

Ordered adversarial-TDD: each task writes the failing test first. No
hardcoded overrides; audio from the local recording folder resolved at
runtime (env override, marker literal never in code); token-only text in
committed tests.

## Phase 1 — synthesis core (pure, fakes only)

- [x] 1.1 RED: `synthesize_overlap_rows` — covered span + both-stream
      decisive votes + per-stream texts → per-voice rows replacing the
      span's mixture rows, traced to source row ids. Adversarial: silent
      stream → no synthesis; hallucinated echo text → guard drops it;
      margin miss on either stream → no synthesis; edge-clamped span → no
      panic; stream row with an original_id absent from the source set →
      rejected (persist would silently drop it — explore finding 2).
- [ ] 1.2 GREEN: pure function + text guards (reuse degenerate-repeat dedup
      and language pin on stream text); adversarial suite green.

## Phase 2 — transcription seam

- [ ] 2.1 `commands.rs` builds the closure over the EXISTING
      `whisper_engine::commands::WHISPER_ENGINE` static (explore finding 1 —
      no new handle; no confidence floor — design thread B: the returned
      confidence is a text-length proxy, anti-correlated with
      hallucination) and passes it as `&dyn Fn(&[f32]) -> Option<String>`
      into the synthesis pass (composition root only;
      hexagonal-port-traits linkage recorded in design.md). RED: the
      synthesis pass inserts AFTER `resolve_duplicate_clusters` — a
      same-span stream pair with token-similar text survives the D4
      resolver (design thread A hazard, adversarial test 10).
- [ ] 2.2 RED: whisper unavailable / separation model missing → render
      byte-identical to today's (degrade channel, existing pin).

## Phase 3 — render + persistence

- [ ] 3.1 RED: immutability — full run leaves `transcript_sources`
      byte-identical; regenerated rendering contains the stream rows.
- [ ] 3.2 GREEN: stream rows carry `speaker_source` from the stream vote;
      persistence round-trip (persist → refetch → badges and text intact).
- [ ] 3.3 Gate extension: ear-truth gate asserts the S16 window renders two
      distinct-badge in-order rows (needles from the local fixture); the
      duplicate scan's exemption is provenance-scoped at GROUP level
      (overlapping-wall groups with intersecting absorbed-row id sets pass;
      disjoint-id overlaps still fail — design thread A, adversarial test
      9); the gate logs the census the Phase 4 clip set draws from: every
      synthesized span's walls, both voices, per-stream text, the trigger
      mass and vote margins that fired it, and a word-loss flag
      (synthesized rows' word count vs the mixture row's) — flag-only,
      never drops (adversarial test 11); all existing pins stay green
      (33.2–38.5 regression window proves no synthesis outside overlap
      spans).
- [ ] 3.4 Live `live_speakers_run`-equivalent persist; DB-layer verification
      (terminal-only rule): S16 rows per-voice, transcript_sources
      untouched.
- [ ] 3.5 Re-pin the gate's render snapshot AFTER 3.4 (explore finding 4:
      the snapshot and the live-DB cross-check must see the same render).

## Phase 4 — the ear

- [ ] 4.1 Ear-calibration clip loop (user-ruled protocol, design thread C):
      the agent reads the full rerun transcript against the original and
      reports whether it makes more or less sense; then hands the user a
      clip set — word-loss-flagged moments first, remainder evenly spaced
      through the meeting, up to ~20 total — each clip with its transcript
      lines and detector evidence. User's ear rules: genuine talk-over?
      lines read right? Rulings tune the detector dials as a general rule
      (never a per-moment skip list).
- [ ] 4.2 S16 graduation: on the user's confirmation, remove the S16
      waiver from the fixture's known_limitations (amendment record
      stays), making the window a hard pin.

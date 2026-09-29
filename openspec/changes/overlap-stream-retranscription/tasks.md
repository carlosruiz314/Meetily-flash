# Tasks: overlap-stream-retranscription

Ordered adversarial-TDD: each task writes the failing test first. No
hardcoded overrides; audio from the local recording folder resolved at
runtime (env override, marker literal never in code); token-only text in
committed tests.

## Phase 1 — synthesis core (pure, fakes only)

- [ ] 1.1 RED: `synthesize_overlap_rows` — covered span + both-stream
      decisive votes + per-stream texts → per-voice rows replacing the
      span's mixture rows, traced to source row ids. Adversarial: silent
      stream → no synthesis; hallucinated echo text → guard drops it;
      margin miss on either stream → no synthesis; edge-clamped span → no
      panic.
- [ ] 1.2 GREEN: pure function + text guards (reuse degenerate-repeat dedup
      and language pin on stream text); adversarial suite green.

## Phase 2 — transcription seam

- [ ] 2.1 `commands.rs` passes the loaded whisper handle as a `&dyn
      Fn(&[f32]) -> Option<String>` into the synthesis pass (composition
      root only; record the hexagonal-port-traits linkage in design.md).
- [ ] 2.2 RED: whisper unavailable / separation model missing → render
      byte-identical to today's (degrade channel, existing pin).

## Phase 3 — render + persistence

- [ ] 3.1 RED: immutability — full run leaves `transcript_sources`
      byte-identical; regenerated rendering contains the stream rows.
- [ ] 3.2 GREEN: stream rows carry `speaker_source` from the stream vote;
      persistence round-trip (persist → refetch → badges and text intact).
- [ ] 3.3 Gate extension: ear-truth gate asserts the S16 window renders two
      distinct-badge in-order rows (needles from the local fixture); all
      existing pins stay green (33.2–38.5 regression window proves no
      synthesis outside overlap spans).
- [ ] 3.4 Live `live_speakers_run`-equivalent persist; DB-layer verification
      (terminal-only rule): S16 rows per-voice, transcript_sources
      untouched.

## Phase 4 — the ear

- [ ] 4.1 Clip loop for the user: audio slices + the new transcript lines
      around 1055–1058 and the 33.2–38.5 regression window. User's ear
      confirms both bars (attribution AND no scrambled sentences).
- [ ] 4.2 S16 graduation: on confirmation, remove the S16 waiver from the
      fixture's known_limitations (amendment record stays), making the
      window a hard pin.

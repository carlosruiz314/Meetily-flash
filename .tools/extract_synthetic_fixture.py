"""Extract the synthetic CI subset for task 1.3 (change hybrid-diarization-engine).

From the recorded frame-mass cache (gate_frame_masses.json, produced by one
production pyannote pass), record small arrays around representative fixture
boundaries so the engine's run/split/attachment rules are asserted in plain
`cargo test` — no audio, no models, no env gate:

- corroborated_34s: the windows adjacent to the 34.66s change — all eight
  covering windows show it; corroboration must ACCEPT.
- seam_rejected_30s: the windows adjacent to the merged track's 30.004s
  flip — a last-writer-wins SEAM artifact (every window decodes 26.4–34.66
  as one speaker); corroboration must REJECT.
- runs_okay_39s: frames around the 39.0–39.93s interjection — the runs must
  match the recorded structure (silence-delimited sub-floor piece).
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CACHE = Path.home() / (
    "Music/meetily-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/"
    "gate_frame_masses.json"
)
OUT = ROOT / "frontend/src-tauri/tests/fixtures/engine_synthetic_cde5c264.json"
SHIFT = 270.0 / 16000.0


def main() -> None:
    fm = json.load(open(CACHE, encoding="utf-8"))
    tracks = {ws: t for ws, t in fm["window_label_tracks"]}

    def windows(starts: list[float]) -> dict:
        return {
            "kind": "windows",
            "window_starts": starts,
            "window_tracks": [tracks[s] for s in starts],
        }

    i0 = int(37.5 / SHIFT)
    i1 = int(43.0 / SHIFT)
    out = {
        "frame_shift_secs": SHIFT,
        "silence_label": 255,
        "cases": {
            "corroborated_34s": windows([33.0, 34.0]),
            "seam_rejected_30s": windows([29.0, 30.0]),
            "runs_okay_39s": {
                "kind": "frames",
                "start_frame": i0,
                "frames": fm["frames"][i0:i1],
            },
        },
    }

    OUT.write_text(json.dumps(out), encoding="utf-8")
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)")


if __name__ == "__main__":
    main()

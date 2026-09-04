"""Locate fixture_clips/*.mp3 cut offsets inside the meeting audio.

Task 1.1 closure: the user answered the clip questions in CLIP-relative time;
converting answers to meeting-absolute fixture pins requires each clip's exact
start offset. The cut commands were not recorded, so this recovers them by
coarse (4 kHz FFT) + fine (16 kHz direct) cross-correlation.

Writes fixture_clips/offsets.json: {clip: {start_s, peak_score, matched}}.
"""
import json
import subprocess
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
CLIPS = ROOT / "fixture_clips"
AUDIO = Path.home() / "Music/meetily-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4"
SR = 16_000
COARSE_SR = 4_000


def decode_pcm(path: Path, sr: int) -> np.ndarray:
    cmd = [
        "ffmpeg", "-v", "error", "-i", str(path), "-vn",
        "-ac", "1", "-ar", str(sr), "-f", "f32le", "-",
    ]
    raw = subprocess.run(cmd, capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype=np.float32).copy()


def downsample(x: np.ndarray, factor: int) -> np.ndarray:
    n = (len(x) // factor) * factor
    return x[:n].reshape(-1, factor).mean(axis=1)


def main() -> None:
    print("decoding meeting audio at 16k...", flush=True)
    meeting = decode_pcm(AUDIO, SR)
    print(f"meeting: {len(meeting)/SR:.1f}s", flush=True)
    meeting_ds = downsample(meeting, SR // COARSE_SR)

    out = {}
    for clip_path in sorted(CLIPS.glob("*.mp3")):
        clip = decode_pcm(clip_path, SR)
        clip_ds = downsample(clip, SR // COARSE_SR)
        # Coarse search at 4 kHz (FFT correlation).
        n = 1 << int(np.ceil(np.log2(len(meeting_ds) + len(clip_ds) - 1)))
        spec_m = np.fft.rfft(meeting_ds, n)
        spec_c = np.fft.rfft(clip_ds, n)
        corr = np.fft.irfft(spec_m * np.conj(spec_c), n)
        # Zero-pad wrap-around: negative lags fold to the end; clip cuts are
        # strictly inside the meeting so lags near 0 are the only candidates.
        lag_ds = int(np.argmax(corr[: len(meeting_ds) - len(clip_ds)]))
        start_coarse = lag_ds * (SR // COARSE_SR)

        # Fine search at 16 kHz around the coarse lag (±1 s).
        best = (-1.0, start_coarse)
        for start in range(max(0, start_coarse - SR), min(len(meeting) - len(clip), start_coarse + SR)):
            seg = meeting[start : start + len(clip)]
            score = float(np.dot(seg, clip))
            if score > best[0]:
                best = (score, start)
        score, start = best
        # Normalized score for sanity (1.0 = perfect waveform match).
        norm = score / (np.linalg.norm(meeting[start : start + len(clip)]) * np.linalg.norm(clip) + 1e-9)
        out[clip_path.name] = {
            "start_s": round(start / SR, 3),
            "end_s": round((start + len(clip)) / SR, 3),
            "corr": round(float(norm), 4),
        }
        print(f"{clip_path.name}: start={start/SR:.3f}s corr={norm:.4f}", flush=True)

    (ROOT / "openspec/changes/hybrid-diarization-engine/fixture-offsets.json").write_text(json.dumps(out, indent=2), encoding="utf-8")
    print("wrote", ROOT / "openspec/changes/hybrid-diarization-engine/fixture-offsets.json")


if __name__ == "__main__":
    sys.exit(main())

"""Build the user's ear-calibration clip set (task 4.1, user-ruled protocol).

Read-only over the production DB + the newest gate census; writes clip wavs
and a verbatim manifest into `fixture_clips/` (gitignored — nothing here
enters the repo). Order: word-loss-flagged regions first, remainder in time
order (the fired regions are already spread across the meeting).

Per clip: before = the source (mixture) rows around the span, after = the
persisted render rows around the span, evidence = census margins / RMS /
stream word counts + the per-region word delta.

Audio comes from the 16 kHz mono f32 samples cache beside the meeting's
audio file (the gate's cache, sha-pinned); each clip is span walls ±6 s.
"""

import json
import os
import re
import sqlite3
import struct
import sys
import wave

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
DB = os.path.join(os.environ["APPDATA"], "com.meetily.ai", "meeting_minutes.sqlite")
CLIPS_DIR = os.path.join(REPO, "fixture_clips")
SNAPSHOT = os.path.join(
    REPO, "frontend", "src-tauri", "tests", "fixtures", "cde5c264_transcripts.json"
)
GATE_RUNS = os.path.join(REPO, "openspec", "changes", "overlap-stream-retranscription", "gate-runs")
CONTEXT_S = 6.0
SR = 16_000


def newest_census():
    logs = [os.path.join(GATE_RUNS, f) for f in os.listdir(GATE_RUNS)]
    logs = [p for p in logs if os.path.isfile(p)]
    path = max(logs, key=os.path.getmtime)
    spans = {}
    pat = re.compile(
        r"CENSUS-SYN span=\[([\d.]+)-([\d.]+)\].*?"
        r"ids=\((Some\(\d+\)|None),(Some\(\d+\)|None)\) "
        r"margins=\(([\d.]+),([\d.]+)\) rms=\(([\d.]+),([\d.]+)\) "
        r"words=\((\d+), (\d+)\).*?verdict=(\w+)"
    )
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            m = pat.search(line)
            if m:
                s0, s1 = float(m.group(1)), float(m.group(2))
                spans[(round(s0, 1), round(s1, 1))] = {
                    "ids": (m.group(3), m.group(4)),
                    "margins": (float(m.group(5)), float(m.group(6))),
                    "rms": (float(m.group(7)), float(m.group(8))),
                    "words": (int(m.group(9)), int(m.group(10))),
                    "verdict": m.group(11),
                }
    return path, spans


def find_samples():
    music = os.path.join(os.environ["USERPROFILE"], "Music")
    for root, dirs, _ in os.walk(music):
        cache = os.path.join(root, "samples_16k.f32")
        if os.path.isfile(cache):
            return cache
    sys.exit("samples_16k.f32 cache not found under Music — run the gate once")


def write_wav(cache_path, s0, s1, out_path):
    with open(cache_path, "rb") as f:
        f.seek(int(max(0.0, s0) * SR) * 4)
        raw = f.read(int((s1 - max(0.0, s0)) * SR) * 4)
    n = len(raw) // 4
    samples = struct.unpack(f"<{n}f", raw[: n * 4])
    pcm = b"".join(
        struct.pack("<h", int(max(-1.0, min(1.0, v)) * 32767)) for v in samples
    )
    with wave.open(out_path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm)


def main():
    meeting = json.load(open(SNAPSHOT, encoding="utf-8"))["meeting"]
    census_path, census = newest_census()
    print(f"census: {os.path.basename(census_path)} ({len(census)} lines)")
    con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)

    spans = con.execute(
        "SELECT MIN(audio_start_time), MAX(audio_end_time), synth_parent "
        "FROM transcripts WHERE meeting_id=? AND synth_atom=1 "
        "GROUP BY synth_parent ORDER BY MIN(audio_start_time)",
        (meeting,),
    ).fetchall()
    if not spans:
        sys.exit("no synth rows in the render — run the live persist first")

    cache = find_samples()
    os.makedirs(CLIPS_DIR, exist_ok=True)

    regions = []
    for (s0, s1, parent) in spans:
        key = (round(s0, 1), round(s1, 1))
        ev = census.get(key) or census.get((round(s0, 2), round(s1, 2))) or {}
        stream_words = sum(ev.get("words", (0, 0)))
        prow = con.execute(
            "SELECT transcript, audio_start_time, audio_end_time "
            "FROM transcript_sources WHERE id=?",
            (parent,),
        ).fetchone()
        parent_dur = max(1e-9, prow[2] - prow[1])
        mixture_in_span = round(len(prow[0].split()) * (s1 - s0) / parent_dur)
        regions.append(
            {
                "s0": s0, "s1": s1, "parent": parent, "ev": ev,
                "stream_words": stream_words,
                "mixture_in_span": mixture_in_span,
                "loss": stream_words < mixture_in_span,
            }
        )
    # protocol: word-loss-flagged first, remainder in time order
    regions.sort(key=lambda r: (not r["loss"], r["s0"]))

    lines = ["# Ear-calibration clips (verbatim — gitignored)", ""]
    for i, r in enumerate(regions, 1):
        s0, s1, parent, ev = r["s0"], r["s1"], r["parent"], r["ev"]
        wav = f"clip-{i:02d}-{int(s0)}s.wav"
        write_wav(cache, s0 - CONTEXT_S, s1 + CONTEXT_S, os.path.join(CLIPS_DIR, wav))

        before = con.execute(
            "SELECT audio_start_time, transcript FROM transcript_sources "
            "WHERE meeting_id=? AND audio_start_time < ? AND audio_end_time > ? "
            "ORDER BY audio_start_time",
            (meeting, s1 + 2, s0 - 2),
        ).fetchall()
        after = con.execute(
            "SELECT audio_start_time, audio_end_time, speaker_label, transcript "
            "FROM transcripts WHERE meeting_id=? AND synth_atom IS NULL "
            "AND audio_start_time < ? AND audio_end_time > ? ORDER BY audio_start_time",
            (meeting, s1 + 2, s0 - 2),
        ).fetchall()

        flag = "WORD-LOSS-FLAG" if r["loss"] else "ok"
        lines += [
            f"## Clip {i:02d} — span [{s0:.2f}-{s1:.2f}]  {os.path.abspath(os.path.join(CLIPS_DIR, wav))}  [{flag}]",
            f"- parent source row ...{parent[-6:]} | census verdict={ev.get('verdict', '?')} "
            f"margins={ev.get('margins', '?')} rms={ev.get('rms', '?')} stream_words={ev.get('words', '?')} "
            f"mixture_words_in_span≈{r['mixture_in_span']}",
            "- BEFORE (mixture rows):",
        ]
        lines += [f"  [{t:8.2f}] {txt}" for t, txt in before]
        lines.append("- AFTER (persisted render):")
        lines += [
            f"  [{a:8.2f}-{b:8.2f}] {lab!r}: {txt}" for a, b, lab, txt in after
        ]
        lines.append("")

    out_md = os.path.join(CLIPS_DIR, "clips_manifest.md")
    with open(out_md, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))
    flagged = [r for r in regions if r["loss"]]
    print(f"{len(regions)} clips + manifest -> {out_md}")
    starts = ", ".join(f"{r['s0']:.0f}s" for r in flagged)
    print(f"word-loss-flagged: {len(flagged)} -> {starts}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

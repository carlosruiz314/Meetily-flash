"""Render the review artifact for the hybrid-diarization-engine change (task 5.4).

Pulls: fixture entries, the latest recorded gate verdicts, the new engine's
turns (from the gate log), and the OLD pipeline's persisted turns from the DB
— and emits a bounded, diff-highlighted review markdown.
"""
import json
import re
import sqlite3
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHANGE = ROOT / "openspec/changes/hybrid-diarization-engine"
GATE_RUNS = sorted(CHANGE.glob("gate-runs/*.log"))
REGIONS = [(0, 45), (130, 180), (2770, 2830)]

OLD_TURNS = {
    # old-pipeline DB turns (pre-change, from the persisted database) per region
}


def latest_gate_log() -> Path:
    return GATE_RUNS[-1]


def parse_gate(log: Path):
    verdicts = {}
    turns = []
    summary = ""
    invariant = ""
    for line in log.read_text(encoding="utf-8", errors="replace").splitlines():
        m = re.match(r"(PASS|FAIL|KNOWN-LIMITATION)(?: \[hold-out\])? (\S+)", line)
        if m:
            verdicts[m.group(2)] = m.group(1)
        if line.startswith("TURN "):
            parts = line.split()
            start, end = parts[1].split("-")
            turns.append((float(start), float(end), parts[2],
                          "cont" in parts, "lowconf" in parts))
        if line.startswith("GATE SUMMARY"):
            summary = line
        if line.startswith("GATE: invariant scan"):
            invariant = line
    return verdicts, turns, summary, invariant


def main() -> None:
    fixture = json.load(open(
        ROOT / "frontend/src-tauri/tests/fixtures/ear_truth_cde5c264.json", encoding="utf-8"))
    log = latest_gate_log()
    verdicts, turns, summary, invariant = parse_gate(log)

    db = sqlite3.connect(f"file:{Path.home() / 'AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite'}?mode=ro", uri=True)
    meeting_id = db.execute(
        "SELECT id FROM meetings WHERE folder_path LIKE '%2026-06-22_16-04-01%'"
    ).fetchone()[0]
    old_rows = db.execute(
        "SELECT audio_start_time, audio_end_time, speaker_label, transcript FROM transcripts "
        "WHERE meeting_id = ? AND speaker_label IS NOT NULL ORDER BY audio_start_time",
        (meeting_id,)).fetchall()

    lines = [
        "# Review — hybrid-diarization-engine (task 5.4)",
        "",
        f"Gate evidence: `{log.name}` — {summary}",
        f"Hard invariant: {invariant}",
        "",
        "Text quality inside turns is whisper's output and out of scope —",
        "this review judges ATTRIBUTION only.",
        "",
        "## Fixture verdicts",
        "",
        "| entry | kind | span | verdict |",
        "|---|---|---|---|",
    ]
    for e in fixture["entries"]:
        v = verdicts.get(e["id"], "?")
        ho = " (hold-out)" if e.get("hold_out") else ""
        lines.append(
            f"| {e['id']}{ho} | {e['kind']} | {e['start_s']:.1f}–{e['end_s']:.1f}s | **{v}** |")

    lines += ["", "## Pinned regions — old pipeline vs new engine", ""]
    entry_by_id = {e["id"]: e for e in fixture["entries"]}
    for lo, hi in REGIONS:
        lines.append(f"### {lo}–{hi}s")
        lines += ["", "**OLD pipeline (persisted turns):**", ""]
        for st, en, lab, txt in old_rows:
            if st < hi and en > lo:
                lines.append(f"- `{st:7.2f}–{en:7.2f}` {lab}: {txt[:90]}")
        lines += ["", "**NEW engine (turns):**", ""]
        for st, en, lab, cont, lowc in turns:
            if st < hi and en > lo:
                tag = (" continues" if cont else "") + (" lowconf" if lowc else "")
                lines.append(f"- `{st:7.2f}–{en:7.2f}` {lab}{tag}")
        pinned = [e for e in fixture["entries"]
                  if e["start_s"] < hi and e["end_s"] > lo]
        if pinned:
            lines += ["", "Entries here: " + ", ".join(
                f"**{e['id']} = {verdicts.get(e['id'], '?')}**" for e in pinned)]
        lines.append("")

    open_entries = [e for e in fixture["entries"] if verdicts.get(e["id"]) == "FAIL"]
    lines += [
        "## Open entries (user chose to leave them open, 2026-09-05)",
        "",
    ]
    for e in open_entries:
        lines.append(f"- **{e['id']}** — {e['note']}")
    lines += [
        "",
        "Root cause: pyannote decodes 26.4–34.66s as one continuous speaker in",
        "every window; no boundary signal exists. Enrollment with independent",
        "reference audio is the path that can revisit these.",
        "",
    ]
    out = CHANGE / "review.md"
    out.write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote {out}")


if __name__ == "__main__":
    sys.exit(main())

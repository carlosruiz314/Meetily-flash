"""Render the hybrid-engine simulation output as a readable transcript.

Associates each DB transcripts row (whisper text atom) with the hybrid turn
that overlaps it most, then writes a markdown preview:
  [MM:SS-MM:SS] Speaker N [overlap flag]
      text...
Speaker names are the DB cluster labels via the sim's midpoint vote mapping.
Rows straddling two hybrid turns bleed whole-text to the majority turn (no
word timings exist to split text mid-row) — the same granularity a production
engine would face.
"""

import json
import os
import sqlite3

MEETING_ID = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323"
TEMP = os.environ["TEMP"]
SIM = os.path.join(TEMP, "cde5c264_hybrid_sim.json")
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "hybrid_transcript_preview.md")

with open(SIM, encoding="utf-8") as f:
    turns = json.load(f)["turns"]

db = os.path.join(os.environ["USERPROFILE"], "AppData", "Roaming", "com.meetily.ai", "meeting_minutes.sqlite")
con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
rows = con.execute(
    "SELECT audio_start_time, audio_end_time, speaker_label, transcript FROM transcripts "
    "WHERE meeting_id = ? ORDER BY audio_start_time",
    (MEETING_ID,),
).fetchall()
con.close()


def mmss(t):
    m, s = divmod(int(t), 60)
    h, m = divmod(m, 60)
    return f"{h:d}:{m:02d}:{s:02d}" if h else f"{m:02d}:{s:02d}"


def overlap(a0, a1, b0, b1):
    return max(0.0, min(a1, b1) - max(a0, b0))


# First pass: per-turn text (row assigned to its max-overlap turn).
assign = [[] for _ in turns]
for r in rows:
    s, e, label, text = r
    best, best_ov = None, 0.0
    for i, t in enumerate(turns):
        ov = overlap(s, e, t["start"], t["end"])
        if ov > best_ov:
            best, best_ov = i, ov
    if best is not None:
        assign[best].append((s, text.strip()))

# Emission pass: textless voiced runs (breaths/laughs/crosstalk noise) are
# dropped BEFORE speaker-run coalescing, so a chain of them never dices one
# speaker's stretch into fragments ("Where | is Ricardo"). Same-cluster text
# turns separated only by dropped runs merge; different-cluster turns keep
# the boundary (a real voice change) and the render marks the continuation.
SENT_END = (".", "!", "?", "…")


def starts_mid_sentence(text):
    stripped = text.lstrip(" \t\n\r\"'()[]{}«»„“-–—*>")
    return bool(stripped) and stripped[0].islower()


emitted = []  # list of dicts: start, end, cluster, overlap_frac, body
pending_gap = False
for i, t in enumerate(turns):
    body = " ".join(tx for _, tx in assign[i] if tx).strip()
    if not body:
        pending_gap = True
        continue
    if (
        emitted
        and pending_gap
        and emitted[-1]["cluster"] == t["cluster"]
    ):
        emitted[-1]["end"] = t["end"]
        emitted[-1]["overlap_frac"] = max(emitted[-1]["overlap_frac"], t["overlap_frac"])
        emitted[-1]["body"] = (emitted[-1]["body"] + " " + body).strip()
    else:
        emitted.append({
            "start": t["start"], "end": t["end"], "cluster": t["cluster"],
            "overlap_frac": t["overlap_frac"], "body": body,
            "db_label": t.get("db_label"),
        })
    pending_gap = False

# Continuation markers: a turn whose text doesn't end sentence-final and is
# followed by text starting mid-sentence gets a trailing ellipsis.
for j in range(len(emitted) - 1):
    a, b = emitted[j]["body"], emitted[j + 1]["body"]
    if not a.rstrip().endswith(SENT_END) and starts_mid_sentence(b):
        emitted[j]["continues"] = True

lines = []
lines.append("# Hybrid engine transcript preview — meeting cde5c264")
lines.append("")
lines.append("Turns = pyannote speech-runs (pause-delimited); labels = TitaNet run-level")
lines.append("embeddings mapped to DB cluster names. `crosstalk NN%` = fraction of frames")
lines.append("where pyannote detects a second simultaneous voice — identity there is")
lines.append("genuinely ambiguous for any local method. A trailing `…` marks a sentence")
lines.append("that continues into the next turn (real overlap/interruption). Textless")
lines.append("voiced runs (breaths/laughs) are dropped before coalescing.")
lines.append("")

for t in emitted:
    flag = ""
    if t["overlap_frac"] >= 0.2:
        flag = f"  `crosstalk {int(round(t['overlap_frac']*100))}%`"
    if t.get("continues"):
        flag += "  `…`"
    name = t["db_label"] or "?"
    lines.append(f"**[{mmss(t['start'])}–{mmss(t['end'])}] {name}**{flag}")
    lines.append("")
    lines.append(f"  {t['body']}")
    lines.append("")

textless = sum(1 for i in range(len(turns)) if not assign[i])
lines.append(f"---")
lines.append(
    f"*{len(emitted)} turns with text shown; {textless} textless voiced runs dropped before coalescing.*"
)

with open(OUT, "w", encoding="utf-8") as f:
    f.write("\n".join(lines))

print(f"wrote {OUT}")
print(f"turns={len(turns)} rows_assigned={sum(1 for a in assign if a)} textless_runs={textless} emitted={len(emitted)}")

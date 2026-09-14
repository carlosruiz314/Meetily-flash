"""Snapshot the live transcript rows of meeting cde5c264 into the gate fixture.

Change `no-split-sentences` task 1.1 (re-run for task 3.4 post-live re-pin).
Read-only over the production DB; writes
`frontend/src-tauri/tests/fixtures/cde5c264_transcripts.json` with the FULL
meeting id and a SHA-256 over the ordered rows.

Canonical hash (MUST stay in sync with the Rust reader in
`tests/ear_truth_gate.rs`): rows ordered by (start_ms, end_ms, id); each row
serialised as `id \\x1f text \\x1f start_ms \\x1f end_ms \\x1f token_json_or_empty`
(start/end truncated toward zero after *1000.0); rows joined by `\\x1e`;
SHA-256 hex over the UTF-8 bytes.
"""

import datetime
import hashlib
import json
import os
import sqlite3
import sys

MEETING = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323"
# Row count is NOT pinned: a live Speakers run legitimately moves it
# (240 pre-run source rows → 220 labeled segments consolidated into 188
# persisted rows). The guard only catches an empty/partial meeting.
MIN_ROWS = 100


def main() -> int:
    repo = os.path.abspath(
        os.path.join(os.path.dirname(__file__), "..", "..", "..", "..")
    )
    db = os.path.join(
        os.environ["APPDATA"], "com.meetily.ai", "meeting_minutes.sqlite"
    )
    out = os.path.join(
        repo, "frontend", "src-tauri", "tests", "fixtures", "cde5c264_transcripts.json"
    )

    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    # align-from-immutable-source task 3.2: the snapshot's subject is the
    # pipeline's ACTUAL INPUT — the immutable `transcript_sources` table — not
    # the rendering rows the pipeline writes. Fall back to `transcripts` only
    # on a pre-migration DB (table absent).
    has_sources = con.execute(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'transcript_sources'"
    ).fetchone()[0]
    table = "transcript_sources" if has_sources else "transcripts"
    print(f"snapshotting from {table}")
    rows = con.execute(
        "SELECT id, transcript, audio_start_time, audio_end_time, token_timestamps "
        f"FROM {table} WHERE meeting_id = ? "
        "ORDER BY audio_start_time ASC, audio_end_time ASC, id ASC",
        (MEETING,),
    ).fetchall()
    con.close()

    if len(rows) < MIN_ROWS:
        print(
            f"ABORT: expected >= {MIN_ROWS} rows for {MEETING}, found {len(rows)} "
            "(the live DB moved — investigate before pinning)",
            file=sys.stderr,
        )
        return 1

    items = [
        {
            "id": rid,
            "text": text,
            "start_ms": int(s * 1000.0),
            "end_ms": int(e * 1000.0),
            "token_timestamps": tok,
        }
        for (rid, text, s, e, tok) in rows
    ]
    items.sort(key=lambda r: (r["start_ms"], r["end_ms"], r["id"]))
    canon = "\x1e".join(
        "\x1f".join(
            [
                r["id"],
                r["text"],
                str(r["start_ms"]),
                str(r["end_ms"]),
                r["token_timestamps"] or "",
            ]
        )
        for r in items
    )
    digest = hashlib.sha256(canon.encode("utf-8")).hexdigest()

    fixture = {
        "meeting": MEETING,
        "captured": datetime.date.today().isoformat(),
        "row_sha256": digest,
        "rows": items,
    }
    with open(out, "w", encoding="utf-8") as f:
        json.dump(fixture, f, ensure_ascii=False, indent=1)
    print(f"pinned {len(items)} rows -> {out}")
    print(f"row_sha256 = {digest}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

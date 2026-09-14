#!/usr/bin/env python3
"""Prototype hallucination detector over transcript rows (no audio).

Validates against the pinned cde5c264 fixtures: flags every row containing
Whisper salad (non-Latin script chars, U+FFFD, token loops, absurd token rate)
and must not flag any row that contains none of those signals.

Usage: python hallucination-detector-probe.py path/to/fixture.json [fixture2.json ...]

Exit code 0 = all fixtures pass the self-consistency check.
"""
import json
import sys
import unicodedata
from collections import Counter

# Non-Latin scripts Whisper hallucinates into English meetings.
SUSPECT_SCRIPTS = {
    "HAN", "HIRAGANA", "KATAKANA", "HANGUL", "CYRILLIC", "GREEK", "THAI",
    "ARABIC", "HEBREW", "DEVANAGARI", "BENGALI", "KHMER", "LAO", "MYANMAR",
    "GEORGIAN", "ARMENIAN", "ETHIOPIC", "TIBETAN", "SINHALA", "TAMIL",
    "TELUGU", "KANNADA", "MALAYALAM", "GUJARATI", "GURMUKHI", "ORIYA",
}
FFFD = "\ufffd"

# Policy thresholds (calibrated on cde5c264 pre-live + current fixtures).
MIN_NON_LATIN_CHARS = 3   # absolute count of non-Latin script chars
MIN_LOOP_RATIO = 0.5      # top repeated token / all tokens ...
MIN_LOOP_COUNT = 10       # ... AND at least this many occurrences of it (a 6-token
                          # "yeah yeah yeah yeah yeah yeah" backchannel is real speech;
                          # "Okay." x37 and "Yes." x50 are degeneration)
MIN_LOOP_TOKENS = 6
ABSURD_WPS = 25.0         # words per second, rows >= 0.8 s


def classify(ch: str) -> str:
    """Unicode script-ish class of one char (unicodedata has no script field)."""
    name = unicodedata.name(ch, "")
    # e.g. 'CJK UNIFIED IDEOGRAPH-4E00', 'HANGUL SYLLABLE GA', 'CYRILLIC SMALL LETTER A'
    first = name.split()[0] if name else ""
    return first


def row_signals(text: str, start_ms: float, end_ms: float) -> dict:
    chars = list(text)
    non_latin = sum(1 for c in chars if classify(c) in SUSPECT_SCRIPTS)
    fffd = text.count(FFFD)
    toks = [t for t in text.split() if t]
    loop_ratio = 0.0
    loop_count = 0
    if len(toks) >= MIN_LOOP_TOKENS:
        top, cnt = Counter(toks).most_common(1)[0]
        loop_ratio = cnt / len(toks)
        loop_count = cnt
    dur_s = (end_ms - start_ms) / 1000.0
    wps = len(toks) / dur_s if dur_s >= 0.8 else 0.0
    return {
        "non_latin": non_latin,
        "fffd": fffd,
        "loop_ratio": round(loop_ratio, 2),
        "loop_count": loop_count,
        "wps": round(wps, 1),
    }


def is_garbage(sig: dict) -> bool:
    return (
        sig["fffd"] >= 1
        or sig["non_latin"] >= MIN_NON_LATIN_CHARS
        or (sig["loop_ratio"] >= MIN_LOOP_RATIO and sig["loop_count"] >= MIN_LOOP_COUNT)
        or sig["wps"] >= ABSURD_WPS
    )


def has_any_signal(sig: dict) -> bool:
    """Ground truth: does the row contain ANY salad signal at all (loosest read)?"""
    return sig["fffd"] >= 1 or sig["non_latin"] >= 1 or sig["loop_ratio"] >= 0.4


def load_rows(path: str):
    with open(path, encoding="utf-8") as f:
        data = json.load(f)
    return data["rows"]


def run(path: str) -> bool:
    rows = load_rows(path)
    flagged = []
    loose = []
    for i, r in enumerate(rows):
        sig = row_signals(r["text"], r["start_ms"], r["end_ms"])
        if is_garbage(sig):
            flagged.append((i, sig, r))
        if has_any_signal(sig):
            loose.append(i)
    print(f"\n=== {path}")
    print(f"rows: {len(rows)}  flagged: {len(flagged)}  rows-with-any-signal: {len(loose)}")
    for i, sig, r in flagged:
        t = (r["start_ms"] / 1000.0)
        print(f"  [{i:3d}] t={t:7.1f}s {sig}  {r['text'][:80]!r}")
    # Self-consistency: no flagged row may lack every salad signal, and the
    # detector must not flag rows whose only 'signal' is a single stray char.
    damaged = [i for i, sig, _r in flagged if not has_any_signal(sig)]
    missed = [i for i in loose if i not in {j for j, _s, _r in flagged}
              and row_signals(rows[i]["text"], rows[i]["start_ms"], rows[i]["end_ms"])["non_latin"] >= MIN_NON_LATIN_CHARS]
    if damaged:
        print(f"  FAIL: flagged rows with no salad evidence: {damaged}")
    if missed:
        print(f"  FAIL: rows with strong salad not flagged: {missed}")
    return not damaged and not missed


if __name__ == "__main__":
    args = sys.argv[1:]
    if "--expectations" in args:
        # Emit a tracked expectations file (flagged row ids keyed by fixture
        # row_sha256) so the Rust fixture gate does not depend on untracked
        # fixture paths alone. Usage: probe.py --expectations OUT.json FIXTURE...
        i = args.index("--expectations")
        dest, fixtures = args[i + 1], args[i + 2:]
        out = {}
        for p in fixtures:
            with open(p, encoding="utf-8") as f:
                meta = json.load(f)
            sigs = [(r, row_signals(r["text"], r["start_ms"], r["end_ms"])) for r in meta["rows"]]
            out[meta["row_sha256"]] = {
                "fixture": p.replace("\\", "/").split("/")[-1],
                "rows_total": len(sigs),
                "flagged": [{"id": r["id"], "start_ms": r["start_ms"], "signals": s}
                            for r, s in sigs if is_garbage(s)],
            }
        with open(dest, "w", encoding="utf-8") as f:
            json.dump(out, f, ensure_ascii=False, indent=1)
        print(f"wrote {dest}")
        sys.exit(0)
    ok = all(run(p) for p in args)
    sys.exit(0 if ok else 1)

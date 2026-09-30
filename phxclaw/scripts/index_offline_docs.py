#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import html
import json
import re
from pathlib import Path

TAG_RE = re.compile(r"<[^>]+>")
SCRIPT_RE = re.compile(r"<(script|style)\b[^>]*>.*?</\1>", re.I | re.S)
TITLE_RE = re.compile(r"<title[^>]*>(.*?)</title>", re.I | re.S)
WS_RE = re.compile(r"\s+")


def html_to_text(raw: str) -> str:
    raw = SCRIPT_RE.sub(" ", raw)
    raw = TAG_RE.sub(" ", raw)
    raw = html.unescape(raw)
    return WS_RE.sub(" ", raw).strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    root = Path(args.root).resolve()
    out = Path(args.out).resolve()
    out.parent.mkdir(parents=True, exist_ok=True)

    count = 0
    with out.open("w", encoding="utf-8") as fp:
        for path in sorted(root.rglob("*.html")):
            try:
                raw = path.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            m = TITLE_RE.search(raw)
            title = html_to_text(m.group(1)) if m else path.stem
            text = html_to_text(raw)
            rel = path.relative_to(root).as_posix()
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            record = {
                "path": rel,
                "title": title,
                "sha256": digest,
                "text": text[:24000],
            }
            fp.write(json.dumps(record, ensure_ascii=False) + "\n")
            count += 1
    print(json.dumps({"indexed": count, "root": str(root), "out": str(out)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

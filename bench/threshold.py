#!/usr/bin/env python3
"""bench/threshold.py — measure what OUTLINE_THRESHOLD_BYTES should be.

The threshold decides when compress_file stops preserving bodies and switches
to a signature outline. It was set to 24 KB by guess (docs/BACKLOG.md), and the
guess was made before outline mode kept signatures whole and docstring
summaries (ADR-007, ADR-008) -- so it predates the thing it is calibrating.

This runs the real release binary over a real corpus, at several thresholds,
and reports what each one costs and buys. It answers one question:

    below what size is outlining not worth the loss?

Usage:
    ./bench/threshold.py                      # this repo's own src/
    ./bench/threshold.py path/to/repo ...     # any corpus you like

Point it at something that is NOT this project. Our own files are unusually
doc-heavy and unusually well formatted, which makes them the least
representative benchmark available.

Every file is compressed twice per threshold sweep: once whole, once outlined,
so the two are compared on identical input rather than across files.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

BINARY = Path(__file__).resolve().parent.parent / "target/release/context-compressor-mcp"

CODE_SUFFIXES = {".py", ".rs", ".ts", ".tsx", ".js", ".jsx", ".go"}
SKIP_PARTS = {
    "node_modules", ".git", "target", ".venv", "dist", "build",
    ".next", "__pycache__", ".obsidian", "site-packages/pip",
}
MAX_BYTES = 8 * 1024 * 1024

# Keep in step with OUTLINE_THRESHOLD_BYTES in src/compress/mod.rs.
CURRENT_DEFAULT = 8192
THRESHOLDS = [2048, 4096, CURRENT_DEFAULT, 16384, 24576, 49152]


def find_files(roots: list[Path], per_root: int = 120) -> list[Path]:
    found: list[Path] = []
    for root in roots:
        if not root.exists():
            print(f"  (skipping missing {root})", file=sys.stderr)
            continue
        picked = 0
        for path in sorted(root.rglob("*")):
            if picked >= per_root:
                break
            if path.suffix not in CODE_SUFFIXES or not path.is_file():
                continue
            # Only skip vendored dirs *below* the given root: pointing a root
            # straight at .venv/site-packages is a deliberate choice.
            if SKIP_PARTS & set(path.relative_to(root).parts):
                continue
            try:
                size = path.stat().st_size
            except OSError:
                continue
            # Files under 1 KB are noise; the threshold question is about big ones.
            if not (1024 < size < MAX_BYTES):
                continue
            found.append(path)
            picked += 1
    return found


def compress(root: Path, files: list[Path], threshold: int | None) -> dict[str, dict]:
    """One binary invocation, one request per file. Returns path -> header fields."""
    lines = []
    for i, path in enumerate(files):
        args: dict[str, object] = {"filePath": str(path.relative_to(root))}
        if threshold is not None:
            args["outlineThreshold"] = threshold
        lines.append(json.dumps({
            "jsonrpc": "2.0", "id": i, "method": "tools/call",
            "params": {"name": "compress_file", "arguments": args},
        }))

    proc = subprocess.run(
        [str(BINARY)], input="\n".join(lines) + "\n",
        capture_output=True, text=True, cwd=root, check=False,
    )

    results: dict[str, dict] = {}
    for line, path in zip(proc.stdout.splitlines(), files):
        try:
            payload = json.loads(line)["result"]
        except (json.JSONDecodeError, KeyError):
            continue
        if payload.get("isError"):
            continue
        text = payload["content"][0]["text"]
        header = text.splitlines()[0] if text else ""
        # // context pack | Python | outline | 441238 -> 28829 bytes (93.5% saved)
        parts = [p.strip() for p in header.lstrip("/ ").split("|")]
        if len(parts) < 4:
            continue
        try:
            sizes = parts[3].split()
            original, compressed = int(sizes[0]), int(sizes[2])
        except (IndexError, ValueError):
            continue
        results[str(path)] = {
            "language": parts[1],
            "mode": parts[2],
            "original": original,
            "compressed": compressed,
            "body": text,
        }
    return results


def bucket(size: int) -> str:
    for limit, label in [(2048, "1-2 KB"), (4096, "2-4 KB"), (8192, "4-8 KB"),
                         (16384, "8-16 KB"), (32768, "16-32 KB"), (65536, "32-64 KB")]:
        if size < limit:
            return label
    return "64 KB+"


def main() -> int:
    if not BINARY.exists():
        print(f"build first: cargo build --release ({BINARY} missing)", file=sys.stderr)
        return 2

    # Defaults to this repo so the script runs anywhere with no setup. The
    # numbers in ADR-010 came from a wider corpus — pass your own paths to
    # reproduce that shape: a large third-party library is the useful case.
    roots = [Path(a).resolve() for a in sys.argv[1:]] or [
        Path(__file__).resolve().parent.parent / "src",
    ]

    print("Collecting corpus...")
    by_root: dict[Path, list[Path]] = {}
    for root in roots:
        files = find_files([root])
        if files:
            by_root[root] = files
    total = sum(len(f) for f in by_root.values())
    if not total:
        print("no files found", file=sys.stderr)
        return 1
    print(f"  {total} files across {len(by_root)} roots\n")

    # --- baseline: every file compressed whole, no outlining -----------------
    whole: dict[str, dict] = {}
    for root, files in by_root.items():
        whole |= compress(root, files, threshold=10**9)

    langs: dict[str, list] = {}
    buckets: dict[str, list] = {}
    for info in whole.values():
        langs.setdefault(info["language"], []).append(info)
        buckets.setdefault(bucket(info["original"]), []).append(info)

    print("=" * 68)
    print("STRUCTURAL COMPRESSION ONLY (no outlining) — what the cheap passes buy")
    print("=" * 68)
    print(f"{'language':<14}{'files':>7}{'original':>12}{'saved':>10}")
    print("-" * 68)
    for lang, items in sorted(langs.items(), key=lambda kv: -len(kv[1])):
        orig = sum(i["original"] for i in items)
        comp = sum(i["compressed"] for i in items)
        print(f"{lang:<14}{len(items):>7}{orig / 1024:>10.0f} KB{100 * (1 - comp / orig):>9.1f}%")

    print()
    print(f"{'file size':<14}{'files':>7}{'original':>12}{'saved':>10}")
    print("-" * 68)
    order = ["1-2 KB", "2-4 KB", "4-8 KB", "8-16 KB", "16-32 KB", "32-64 KB", "64 KB+"]
    for label in order:
        items = buckets.get(label)
        if not items:
            continue
        orig = sum(i["original"] for i in items)
        comp = sum(i["compressed"] for i in items)
        print(f"{label:<14}{len(items):>7}{orig / 1024:>10.0f} KB{100 * (1 - comp / orig):>9.1f}%")

    # --- sweep ---------------------------------------------------------------
    print()
    print("=" * 68)
    print("THRESHOLD SWEEP — what outlining adds, and what it costs")
    print("=" * 68)
    print(f"{'threshold':>10}{'outlined':>10}{'total saved':>14}{'body loss':>12}{'per outlined':>14}")
    print("-" * 68)

    total_original = sum(i["original"] for i in whole.values())
    for threshold in THRESHOLDS:
        merged: dict[str, dict] = {}
        for root, files in by_root.items():
            merged |= compress(root, files, threshold=threshold)
        if not merged:
            continue
        outlined = [k for k, v in merged.items() if v["mode"] == "outline"]
        total_compressed = sum(v["compressed"] for v in merged.values())
        # Bytes given up by outlining, versus keeping those same files whole.
        loss = sum(whole[k]["compressed"] - merged[k]["compressed"]
                   for k in outlined if k in whole)
        per = loss / len(outlined) if outlined else 0
        marker = "  <- current default" if threshold == CURRENT_DEFAULT else ""
        print(f"{threshold:>10}{len(outlined):>10}"
              f"{100 * (1 - total_compressed / total_original):>13.1f}%"
              f"{loss / 1024:>10.0f} KB{per / 1024:>12.1f} KB{marker}")

    print()
    print("Reading this: 'total saved' is the whole corpus. 'body loss' is what")
    print("outlining threw away versus compressing those same files whole —")
    print("recoverable with get_symbol, one call per body actually needed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

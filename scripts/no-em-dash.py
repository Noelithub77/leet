#!/usr/bin/env python3
import subprocess
import sys
from pathlib import Path

forbidden = chr(0x2014)
extensions = {".rs", ".toml", ".md", ".html", ".css", ".js", ".ts"}
files = subprocess.run(
    ["git", "ls-files", "-co", "--exclude-standard"],
    check=True,
    capture_output=True,
    text=True,
).stdout.splitlines()
matches = []
for name in files:
    path = Path(name)
    if path.suffix not in extensions or "target" in path.parts:
        continue
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError:
        continue
    matches.extend(f"{name}:{number}" for number, line in enumerate(lines, 1) if forbidden in line)

if matches:
    print("no-em-dash-copy: replace the em dash character in:\n" + "\n".join(matches), file=sys.stderr)
    sys.exit(1)

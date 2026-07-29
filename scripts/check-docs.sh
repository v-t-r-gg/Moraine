#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

CLI="${1:-target/debug/moraine}"
test -x "$CLI" || {
  echo "documentation check requires a built moraine CLI: $CLI" >&2
  exit 1
}

python3 - <<'PY'
from pathlib import Path
import re
import sys

root = Path.cwd()
docs = [
    Path(line)
    for line in __import__("subprocess")
    .check_output(["rg", "--files", "-g", "*.md"], text=True)
    .splitlines()
]

failures = []
link_re = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
for rel in docs:
    text = (root / rel).read_text(encoding="utf-8")
    for raw in link_re.findall(text):
        target = raw.split("#", 1)[0].strip()
        if not target or re.match(r"^(https?://|mailto:)", target):
            continue
        target = target.strip("<>")
        resolved = (root / rel.parent / target).resolve()
        if not resolved.exists():
            failures.append(f"{rel}: broken link {raw}")

deleted = [
    "DEVELOPMENT_BLUEPRINT_ALIGNED",
    "Moraine_Development_Blueprint",
    "docs/QUICKSTART.md",
    "docs/MCP.md",
    "docs/REDACTION.md",
    "docs/C3_SURFACE_FREEZE.md",
    "docs/OUTSIDE_USER_INSTALL_REPORT.md",
    "docs/DEMO.md",
    "docs/integrations/codex/",
    "docs/evaluation/2026-07/",
]
for rel in docs:
    text = (root / rel).read_text(encoding="utf-8")
    for stale in deleted:
        if stale in text:
            failures.append(f"{rel}: references deleted authority {stale}")

phrases = [
    "c3 focus",
    "w1 closes when",
    "m5 current",
    "five tools only",
    "windows compilation not yet proven",
]
for rel in docs:
    if rel.parts[:2] == ("docs", "adr"):
        continue
    text = (root / rel).read_text(encoding="utf-8").lower()
    for phrase in phrases:
        if phrase in text:
            failures.append(f"{rel}: stale status phrase {phrase!r}")

schema_source = (root / "crates/moraine-core/src/run_meta.rs").read_text()
match = re.search(r"pub const SCHEMA_VERSION: u32 = (\d+);", schema_source)
if not match:
    failures.append("could not read SCHEMA_VERSION from moraine-core")
else:
    current = match.group(1)
    for rel in docs:
        if rel.parts[:2] == ("docs", "adr"):
            continue
        text = (root / rel).read_text(encoding="utf-8")
        for claimed in re.findall(r"schema[- ]v(\d+)", text, flags=re.I):
            if claimed != current:
                failures.append(
                    f"{rel}: schema-v{claimed} disagrees with writable schema v{current}"
                )

cargo_files = list(root.glob("crates/*/Cargo.toml"))
for cargo in cargo_files:
    if re.search(r"(?m)^readme\s*=", cargo.read_text()):
        failures.append(f"{cargo.relative_to(root)}: package metadata references a crate README")

if failures:
    print("\n".join(failures), file=sys.stderr)
    sys.exit(1)
PY

for args in \
  "--help" \
  "setup --help" \
  "doctor --help" \
  "project init --help" \
  "run start --help" \
  "open --help" \
  "service status --help" \
  "integrate codex --help"
do
  # shellcheck disable=SC2086
  "$CLI" $args >/dev/null
done

grep -q 'tool_names' crates/moraine-mcp/src/lib.rs
! rg -n 'five tools only|start, show, checkpoint, ready, resume' \
  README.md VISION.md ARCHITECTURE.md ROADMAP.md CONTRIBUTING.md SECURITY.md docs

for file in README.md INSTALL.md SECURITY.md TROUBLESHOOTING.md CODEX.md LICENSE
do
  grep -q "share/documentation/$file" scripts/build-linux-release.sh
done
! grep -q 'share/documentation/REDACTION.md' scripts/build-linux-release.sh

echo "documentation contracts ok"

#!/usr/bin/env python3
"""Write suite manifest.json into a staging directory."""
import datetime
import json
import os
import sys
from pathlib import Path

stage = Path(sys.argv[1])
version = os.environ.get("VERSION") or "0.1.0"
commit = os.environ.get("MORAINE_GIT_COMMIT") or "unknown"
target = os.environ.get("MORAINE_TARGET_TRIPLE") or "x86_64-unknown-linux-gnu"
has_app = (stage / "bin" / "moraine-app").is_file()
manifest = {
    "product": "Moraine",
    "version": version,
    "gitCommit": commit,
    "buildTimestamp": datetime.datetime.now(datetime.timezone.utc)
    .replace(microsecond=0)
    .isoformat()
    .replace("+00:00", "Z"),
    "target": target,
    "profile": os.environ.get("MORAINE_BUILD_PROFILE") or "release",
    "schema": {
        "minimumReadable": 3,
        "maximumReadable": 6,
        "currentWritable": 6,
    },
    "serviceProtocolVersion": 1,
    "mcpImplementationVersion": 1,
    "components": {
        "cli": version,
        "service": version,
        "desktop": version if has_app else "missing",
    },
}
(stage / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(json.dumps(manifest, indent=2))

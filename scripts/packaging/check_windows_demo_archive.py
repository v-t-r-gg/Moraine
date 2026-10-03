#!/usr/bin/env python3
"""Check a Windows demo zip for the staged-suite layout.

The checker does not build binaries. ``--self-test`` writes a synthetic zip
and a zip that must fail. Hosted CI may run the self-test. A real
``moraine-*-windows-x86_64.zip`` is produced only by
``scripts/build-windows-demo.ps1`` on Windows.
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
import zipfile
from pathlib import Path

REQUIRED = (
    "bin/moraine.exe",
    "bin/moraine-service.exe",
    "bin/moraine-app.exe",
    "manifest.json",
    "DEMO.md",
    "stage-windows-demo.ps1",
    "LICENSE",
    "NOTICES",
    "examples/demo-project/src/greet.py",
    "examples/demo-project/EXPECTED_RUN_SHAPE.md",
)

CLAIM = "staged suite, installer unsupported, acceptance pending"


def _names(zf: zipfile.ZipFile) -> list[str]:
    names = []
    for info in zf.infolist():
        name = info.filename.replace("\\", "/")
        if name.endswith("/"):
            continue
        names.append(name)
    return names


def _relative(names: list[str]) -> list[str]:
    tops = {name.split("/", 1)[0] for name in names}
    if "manifest.json" in tops or "bin" in tops:
        return names
    roots = {name.split("/", 1)[0] for name in names if "/" in name}
    if len(roots) != 1:
        raise SystemExit(
            "expected bin/ and manifest.json at the zip root or under one directory, "
            f"found {sorted(tops)}"
        )
    root = next(iter(roots))
    prefix = root + "/"
    return [name[len(prefix) :] for name in names if name.startswith(prefix)]


def _is_archive_manifest(filename: str) -> bool:
    name = filename.replace("\\", "/").rstrip("/")
    if name.endswith("share/moraine/manifest.json"):
        return False
    parts = name.split("/")
    return parts[-1] == "manifest.json" and len(parts) <= 2


def check_zip(path: Path) -> None:
    if not path.is_file():
        raise SystemExit(f"archive not found: {path}")
    with zipfile.ZipFile(path) as zf:
        relative = _relative(_names(zf))
        missing = [item for item in REQUIRED if item not in relative]
        if missing:
            raise SystemExit("windows demo archive missing: " + ", ".join(missing))
        for name in relative:
            base = name.rsplit("/", 1)[-1].lower()
            if "moraine-server" in base:
                raise SystemExit(f"moraine-server must not be in the demo zip: {name}")
            if base.endswith(".msi"):
                raise SystemExit(f"installer package must not be in the demo zip: {name}")
        archive_manifests = [
            info.filename
            for info in zf.infolist()
            if _is_archive_manifest(info.filename)
        ]
        if not archive_manifests:
            raise SystemExit("manifest.json missing at archive root")
        manifest = json.loads(zf.read(archive_manifests[0]))
        version = manifest.get("version")
        components = manifest.get("components") or {}
        for key in ("cli", "service", "desktop"):
            if components.get(key) != version:
                raise SystemExit(
                    f"component {key}={components.get(key)!r} does not match version {version!r}"
                )
        if components.get("desktop") in (None, "", "missing"):
            raise SystemExit("desktop component is missing")
        demo_members = [
            info.filename
            for info in zf.infolist()
            if info.filename.replace("\\", "/").rstrip("/").endswith("DEMO.md")
        ]
        if not demo_members:
            raise SystemExit("DEMO.md missing")
        demo = zf.read(demo_members[0]).decode("utf-8")
        if CLAIM not in demo:
            raise SystemExit(f"DEMO.md must contain {CLAIM!r}")
    print(f"windows demo archive ok: {path}")


def _write_sample(path: Path, *, desktop: str, include_server: bool = False) -> None:
    manifest = {
        "product": "Moraine",
        "version": "0.1.0",
        "components": {
            "cli": "0.1.0",
            "service": "0.1.0",
            "desktop": desktop,
        },
    }
    demo = (
        "staged suite, installer unsupported, acceptance pending\n"
        "Windows Product Ready remains No.\n"
    )
    with zipfile.ZipFile(path, "w") as zf:
        root = "moraine-0.1.0-windows-x86_64"
        for name in REQUIRED:
            payload = demo.encode() if name == "DEMO.md" else b"sample"
            if name == "manifest.json":
                payload = (json.dumps(manifest) + "\n").encode()
            zf.writestr(f"{root}/{name}", payload)
        if include_server:
            zf.writestr(f"{root}/bin/moraine-server.exe", b"no")


def self_test() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        good = root / "good.zip"
        bad_desktop = root / "bad-desktop.zip"
        bad_server = root / "bad-server.zip"
        _write_sample(good, desktop="0.1.0")
        check_zip(good)
        _write_sample(bad_desktop, desktop="missing")
        try:
            check_zip(bad_desktop)
        except SystemExit as exc:
            if "desktop" not in str(exc):
                raise
        else:
            raise SystemExit("self-test: missing desktop was accepted")
        _write_sample(bad_server, desktop="0.1.0", include_server=True)
        try:
            check_zip(bad_server)
        except SystemExit as exc:
            if "moraine-server" not in str(exc):
                raise
        else:
            raise SystemExit("self-test: moraine-server was accepted")
    print("windows demo archive self-test ok")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("archive", nargs="?", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.archive is None:
        raise SystemExit("pass a zip path or --self-test")
    check_zip(args.archive)


if __name__ == "__main__":
    main()

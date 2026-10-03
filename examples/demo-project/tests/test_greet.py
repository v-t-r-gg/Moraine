"""Failing check for the Windows demo. Python 3 runs it. No network."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "src"))

from greet import greet


def main() -> int:
    got = greet("moraine")
    expected = "hello, moraine"
    if got != expected:
        print(f"FAIL: greet returned {got!r}, expected {expected!r}")
        return 1
    print("ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

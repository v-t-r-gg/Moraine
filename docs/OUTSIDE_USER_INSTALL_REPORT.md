# Outside-user installation report (template)

Use this form for one **external** or clean-machine install of the C2/C3 Linux suite.

## Environment

| Field | Value |
|-------|--------|
| Date | 2026-07-21 |
| Reporter | Internal dry-run (C3 lifecycle smoke) — **not** a true outside user |
| Distro / version (`/etc/os-release`) | Arch Linux (host); Ubuntu 24.04 base via bwrap in C2 evidence |
| Arch (`uname -m`) | x86_64 |
| Desktop environment | Headless for automated smoke |
| systemd user available? | Unit written; clean HOME lacks user bus |
| How obtained | Built from `main`/C3 branch release bins |
| Bundle filename + SHA-256 | See CI artifact `moraine-linux-x86_64-suite` |

## Steps performed

- [x] Extracted / staged suite (no source on PATH for smoke)
- [x] `./install.sh` (via packaging scripts)
- [x] PATH = suite bin only (+ system)
- [x] `moraine version --json`
- [x] Service binary start (direct; not `systemctl --user` in temp HOME)
- [x] `moraine project init` on unrelated path
- [ ] Desktop launched (headless — GUI not exercised this report)
- [x] Uninstall; ledger retained (`C3_LIFECYCLE_SMOKE_OK`)

## Results

| Check | Pass? | Notes |
|-------|-------|-------|
| Install without root | yes | user prefix |
| Unit ExecStart absolute libexec | yes | |
| No `.cargo` in unit | yes | |
| Spool while service down | yes | event file created |
| Restart processes spool | yes | status after restart |
| Uninstall keeps `.moraine` | yes | keep.txt retained |
| True outside user | **no** | still needed for C3 acceptance |

## Blockers / confusion

True outside-user graphical install still required. Automated smoke covers install/reinstall/spool/uninstall only.

## Recommendation

- [x] Ready for wider dogfood (suite install path)
- [ ] Needs true outside-user report before calling C3 fully closed

---

*Mark “internal” if not a true outside user — this file currently is internal.*

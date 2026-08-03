# W2-E; Windows 11 standard-user runtime acceptance

**Disposition: NOT EXECUTED**

This is an acceptance record for a real Windows 11 standard-user graphical
session. It is **not** an installation guide and **not** a substitute for hosted
CI. Public support claims in README / ARCHITECTURE / ROADMAP must remain
unchanged until disposition is **PASSED** for every mandatory gate.

Operator harness: [`scripts/windows-w2e/`](../../scripts/windows-w2e/README.md)

Supporting artifacts: [`docs/evidence/windows-w2e/`](windows-w2e/)

---

## Identity

| Field | Value |
|---|---|
| Milestone | W2-E |
| Tested commit | `aaa00f835c4b53381e0914023751c23d9509e0f0` (base; re-record after any fix merge) |
| Acceptance date | _pending live session_ |
| Windows edition | _pending_ |
| Windows version / OS build | _pending_ |
| Architecture | _pending (must be x86-64)_ |
| Environment | _physical or virtual — record which_ |
| Account privilege class | Account A: standard local user (non-admin); Account B: separate standard local user |
| Suite staging | `%LOCALAPPDATA%\Programs\Moraine` (manual stage; not installer) |
| Binary SHA-256 | see `windows-w2e/binary-hashes.txt` after staging |
| WebView2 version | _pending_ |
| Codex version | _pending_ |

---

## Evidence classes

| Class | Role in W2-E |
|---|---|
| Automated CI | Compilation, current-account APIs, Task Scheduler lifecycle, pipe contracts, CLI ProductCapture, frontend & Tauri paths — **already on `main`; insufficient alone** |
| CLI evidence | version/doctor/service/self-test JSON under standard-user token |
| Graphical evidence | Onboarding, health repair, rollback, run discovery screenshots (redacted) |
| Sign-in / restart | Login autostart after sign-out and/or reboot |
| Cross-account | Account B write-only probe denied against Account A’s live pipe |
| Manual observation | No console window, no UAC, process identity, desktop closed capture |

---

## Mandatory gates

Status legend: `pending` · `pass` · `fail`

| # | Gate | Expected | Observed | Status | Evidence class |
|---|---|---|---|---|---|
| 1 | Environment recorded | Edition, build, arch, physical/virtual, commit, hashes, WebView2, Codex, date | | pending | CLI / manual |
| 2 | Account A standard user | Non-admin token; integrity & groups recorded; no elevation | | pending | CLI |
| 3 | Clean machine prestate | No Moraine task, process, pipe, suite, acceptance project, or inherited Moraine env; UAC on | | pending | CLI / manual |
| 4 | Manual suite staging | Coherent suite in Account A-owned dir; writable; no admin; no registry/PATH install | | pending | CLI |
| 5 | Preflight CLI | `version`/`doctor`/`service status` coherent; install unsupported; runtime unregistered; no UAC | | pending | CLI |
| 6 | Native desktop launch | `moraine-app.exe` opens from Explorer; onboarding; not UnsupportedPlatform; no UAC | | pending | Graphical |
| 7 | Graphical onboarding | Disposable project with spaces & non-ASCII; plan apply; Ready; TS registration; pipe ready; ProductCapture run | | pending | Graphical + CLI |
| 8 | No service console | `moraine-service.exe` has no visible console window | | pending | Manual |
| 9 | Runtime ownership | Backend Task Scheduler; SID-scoped least privilege; interactive token; Account A principal; app logs | | pending | CLI |
| 10 | Real Codex capture | Genuine session materializes discoverable run distinct from self-test | | pending | Graphical + manual |
| 11 | Capture without desktop | Desktop closed; new Codex activity discovered on reopen | | pending | Graphical + manual |
| 12 | Demand lifecycle | stop / start / restart restore single clean runtime; no elevation; no console | | pending | CLI |
| 13 | Login autostart | After sign-out or restart: service up ≤60s without desktop or UAC | | pending | Sign-in / restart |
| 14 | Cross-account denial | Account B access denied to live Account A pipe (not merely not-found); A remains healthy | | pending | Cross-account |
| 15 | Health: stopped runtime | Health proposes Start (not Install); Fix → ready | | pending | Graphical |
| 16 | Health: registration drift | Deliberate product-owned drift → registration repair → ready; capture works | | pending | Graphical + CLI |
| 17 | Graphical rollback | Port 33111 occupied post-uninstall; onboarding → RolledBack; exact prestate restored; ledger kept | | pending | Graphical + CLI |
| 18 | Recovery after rollback | Clear port; setup reaches Ready; retained ledger ok; capture works | | pending | Graphical + CLI |
| 19 | Uninstall | Task & process & pipe gone; ledgers retained; desktop can open runs read-only; no elevation | | pending | CLI + Graphical |
| 20 | Evidence hygiene | No passwords, full SIDs, usernames, private prompts, or raw runs by default | | pending | Manual |
| 21 | Repository CI | All required checks green on acceptance branch | | pending | Automated CI |
| 22 | Claim boundary | Installation remains unsupported; no untested arch claims | | pending | Docs |

---

## Observed results (fill during live session)

### Preflight

_Paste or summarize redacted `version` / `doctor` / `service status` outcomes._

### Onboarding

_Project path category (redacted), plan apply outcome, Ready confirmation._

### Runtime & security

_Backend, autostart, capture readiness, process observations, redacted scope id / pipe path category._

### Codex capture

_Mechanical coverage honesty; desktop-closed capture confirmation._

### Lifecycle

_Stop / start / restart observations._

### Login autostart

_Sign-out, restart, or both; timing; status & self-test._

### Cross-account pipe denial

_Probe outcome must be access denied with pipe present._

### Health, repair, rollback

_Pre-drift / drifted / repaired fingerprints where available; RolledBack UI; recovery Ready._

### Uninstall

_Task absence; ledger presence; read-only desktop._

---

## Limitations

* Windows installer, upgrade, signing, and WinGet are **out of scope** (W3).
* Only the recorded Windows 11 x86-64 build is evidenced; other architectures are unclaimed.
* Hosted `windows-latest` CI runs as an administrator account and cannot prove this milestone.
* Manual staging is acceptance tooling, not a supported user installation workflow.

## Defects found

_None recorded — live session not executed._

| Gate | Defect | Fix PR | Regression | Re-test commit |
|---|---|---|---|---|
| — | — | — | — | — |

## Corrective PRs

_None — live session not executed._

## Final disposition

```text
NOT EXECUTED
```

W2-E does not pass when defects are only documented. Failed product gates require
a fix PR from `main`, merge through CI, and a full re-run from a clean Windows
snapshot against the new commit.

Public documentation promotion (README capability table, ARCHITECTURE support
paragraph, ROADMAP W2→Completed / W3→Current, DEVELOPMENT_BLUEPRINT starting
point) is **blocked** until disposition is **PASSED**.

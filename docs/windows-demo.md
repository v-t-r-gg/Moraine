# Windows demo

staged suite, installer unsupported, acceptance pending

This showing is one Windows 11 standard-user session: unzip, stage, set up,
run Codex, and open the desktop on a real run. No UAC prompt. No crash.
That is not W2-E. Login autostart, repair, rollback, and Account B pipe
denial stay `not_executed`. Do not spend the rehearsal on those gates.

Windows Product Ready remains No. A supported Windows installer does not
exist. This archive is a manually staged suite for a Windows 11 x86-64
standard user. It is not W3.

Two machines. The presenter stays on the Linux product and only streams.
The viewer follows this page on Windows. After unzip, the viewer steps are
meant to fit in about 15 minutes. The archive works offline. The only
external dependency is the coding agent. The desktop on this branch is the
review workspace: Projects, Runs, Review, with the fidelity panel.

Rehearsal agent: not rehearsed on Windows in the session that wrote this
script. No operator named an agent. The steps below use Codex. Claude Code
is the same setup with `moraine integrate claude-code` instead of
`moraine integrate codex`. Codex can observe tool activity. Claude Code
capture is lifecycle hooks, so the fidelity report keeps tool activity
`not_supported`. Do not describe Claude tool calls as captured.

The suite prefix is the path `SuitePaths` already discovers:
`%LOCALAPPDATA%\Moraine`. `moraine.exe`, `moraine-service.exe`, and
`moraine-app.exe` live in that directory. The manifest is
`%LOCALAPPDATA%\Moraine\share\moraine\manifest.json`. There is no
`suite\bin` install root.

## 1. Unzip and stage

Unzip `moraine-<version>-windows-x86_64.zip`. In a non-elevated PowerShell
window:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\stage-windows-demo.ps1
```

The script stops if the token is elevated. It does not write Program Files,
Windows, or HKLM. It copies the three executables and the manifest into the
prefix above, prepends that directory to the user PATH when the entry is
absent, and prints three commands. It does not run setup.

Open a new PowerShell window so the user PATH is visible.

```powershell
moraine --version
```

Expect the archive version and the git commit embedded at build time. A
dirty source tree embeds a `-dirty` suffix. Then:

```powershell
moraine doctor
```

Expect a coherent suite when the prefix contains the manifest and binaries.
Expect the installer to be described as unsupported. Expect no Product Ready
claim. Doctor must not tell you to run `./install.sh`.

After you run `moraine doctor`, quote the `id` values from that output. The
implementation's Windows report includes `windows.installer` and
`windows.product_ready_claim`. If those ids are absent, stop. The binary and
this script disagree.

Recorded 2026-10-04 on the staged suite, standard user `demo`, no elevation.
`moraine doctor` exited 0. `windows.installer` was `info`: observed
`user_installation=unsupported`, expected `no supported Windows installer`,
remediation `stage the demo suite; no installer is available.`
`windows.product_ready_claim` was `info`: observed `product_ready_claim=no`.
The other ids were `suite.cli_path` (info), `suite.manifest`,
`suite.components`, `suite.cli_match`, `path.candidate.0`, `path.count`,
`service.binary`, `runtime.backend`, `runtime.registration`,
`runtime.capture`, `runtime.last_result` (info, task result `267011`),
`windows.task_registration`, `windows.application_logs`, `service.online`,
`desktop.binary`, and `desktop.registration` (info). With capture running,
`runtime.capture` and `service.online` passed. Product Ready remains No.

If the suite is missing, doctor fails closed with: stage the demo suite; no
installer is available.

## 2. Project and agent

From the new window, using the sample next to this script in the unzipped
archive:

```powershell
moraine project init .\examples\demo-project
moraine integrate codex --project .\examples\demo-project
moraine setup
```

`moraine project init` creates `.moraine/` in the sample. The sample does
not ship a pre-baked run. `examples/run-bundle` in the Moraine source tree
is a fixture for the file shape, not this demo's result. See
`examples/demo-project/EXPECTED_RUN_SHAPE.md`.

For Claude Code, replace the integrate command:

```powershell
moraine integrate claude-code --project .\examples\demo-project
```

`moraine setup` registers the current-user Task Scheduler task and starts
capture. Expect no UAC prompt. If a UAC prompt appears, stop. That is a demo
blocker. Do not approve elevation.

## 3. One agent run

Start Codex in a fresh `examples\demo-project` (do not reuse a preserved
ledger unless you intend to append). The Moraine hook context names the host
session id. Paste:

```text
Call Moraine run_start with sessionId set to the host session id from the
Moraine hook context. Fix src/greet.py so tests/test_greet.py passes.
Checkpoint the change, then call run_ready. Record the work. Do not ask
Moraine to approve a merge, deployment, or release.
```

Python 3 can run the check locally:

```powershell
python .\examples\demo-project\tests\test_greet.py
```

Expect one run id. The UUID is the `Run ID` line in `.moraine\runs\<run>.md`.
`moraine run coverage` rejects the file name. Pass that UUID:

```powershell
moraine run coverage <RUN_UUID> --project .\examples\demo-project
```

Coverage of that same UUID should show session lifecycle observed, prompt
activity observed, and at least one checkpoint. Tool activity `not_observed`
is a valid report when the session recorded no tool event. Do not mark it
observed. `moraine open` is optional and is not part of this demo. Do not
launch `moraine-app.exe`.

Omitting `sessionId` still creates a run. That run is a different id from the
hook's provisional run, and coverage of it keeps session lifecycle and prompt
activity `unknown`. An earlier standard-user session hit that split:
checkpoint run `76dbc89a-dfca-4902-a35e-926653a613fe` and provisional run
`0303e164-b184-4862-9325-869cc7c7b3c4`.

On Claude Code, tool activity stays `not_supported`.

The coverage transcript for the session-id rehearsal is filled from the SSH
session that runs this archive. Product Ready remains No.

## 4. Service down, then back

Stop capture, then start a short follow-up that still exits normally. The
hook must not fail the agent. A stopped Moraine service stays
non-disruptive. `moraine doctor` should show capture not ready. Start
capture again and leave the project ledger on disk.

```powershell
moraine service stop
moraine doctor
moraine service start
moraine doctor
```

Recorded in the same session, after the checkpoint run:

```powershell
moraine service stop
```

A follow-up Codex prompt exited 0 while capture was stopped. The hooks
completed. `moraine doctor` then exited 0 with `runtime.capture` and
`service.online` at warn (`running=false`). `moraine service start` brought
capture back: a later SSH still saw HTTP 200 on `127.0.0.1:33111/status`,
`captureReady` true, and git commit
`c4095dcb17cf20014c5471c1ab4861f128e3007d`. The three run files that existed
before the stop had the same SHA-256 after the start. The follow-up's
spooled hooks were applied after start and wrote provisional run
`1c90b60a-e707-48f6-9ac7-f50a151d0377`. Do not run `moraine project init`
a second time.

## 5. Demo uninstall

```powershell
moraine service stop
moraine service uninstall
```

`moraine service uninstall` removes the current-user Task Scheduler task
through the existing provision path. Then remove the user PATH entry for
`%LOCALAPPDATA%\Moraine` if you added it, and delete that suite directory
only.

Leave every project `.moraine/` directory in place, including the sample.
Uninstall and rollback do not delete project ledgers.

## Limitations

* Staged suite, installer unsupported, acceptance pending. No signed
  installer, MSIX, WiX, WinGet, or Authenticode.
* Windows Product Ready remains No. Compile support is not runtime support.
  Hosted CI and the smoke scripts are not standard-user acceptance.
* W2-E was not executed. Login autostart, repair, rollback, and Account B
  pipe denial stay `not_executed` for this showing. Do not rehearse them.
  A unit test does not prove cross-account denial.
* Claude Code capture is lifecycle hooks. Tool activity stays
  `not_supported`. Codex tool activity is a different adapter.
* Redaction is not erasure. Raw sidecars, git history, logs, and backups
  can still hold text that ordinary views withhold.
* One local user. Do not expose diagnostics off loopback. Do not publish
  the diagnostics listener. Capture stays the per-user named pipe. Do not
  switch capture to TCP or HTTP.
* The archive does not include the collaboration relay.
* Moraine does not approve merges, deployments, or releases.

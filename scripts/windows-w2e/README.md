# W2-E Windows 11 standard-user acceptance (operator)

This directory is the **operator harness** for W2-E. It does not replace the
acceptance authority in the milestone brief. It automates evidence collection
that can be automated and records where manual observation is required.

## Non-goals

These scripts do not:

* install Moraine for end users;
* claim runtime support on their own;
* run elevated;
* modify Account B’s profile beyond a temporary probe;
* commit secrets, full SIDs, usernames, or raw `.moraine` runs.

## Prerequisites

* Windows 11 x86-64 with a real interactive desktop (physical or VM).
* Account A: standard local user (not Administrators).
* Account B: separate standard local user for pipe denial.
* Coherent W2 suite binaries built from the acceptance commit:
  * `moraine.exe`
  * `moraine-service.exe`
  * `moraine-app.exe`
  * frontend assets already embedded in `moraine-app.exe` (Tauri)
* Real Codex installation supported by the product adapter.
* UAC enabled; clean machine or clean snapshot for Account A Moraine state.

## Recommended flow

1. Sign in as **Account A** (non-elevated).
2. Build or copy the suite, then stage:

   ```powershell
   .\scripts\windows-w2e\stage-suite.ps1 -SourceDir <built suite dir>
   ```

3. Open a **new** non-elevated PowerShell and load the session environment:

   ```powershell
   . .\scripts\windows-w2e\session-env.ps1
   ```

4. Collect environment & preflight:

   ```powershell
   .\scripts\windows-w2e\collect-environment.ps1
   .\scripts\windows-w2e\collect-cli.ps1 -Phase preflight
   ```

5. Perform graphical onboarding, real Codex capture, health/repair/rollback, and
   login autostart **manually** per the milestone gates. Save redacted
   screenshots under `docs/evidence/windows-w2e/screenshots/`.

6. Collect runtime CLI evidence after onboarding:

   ```powershell
   .\scripts\windows-w2e\collect-cli.ps1 -Phase runtime -ProjectPath "<project>"
   .\scripts\windows-w2e\export-task-evidence.ps1
   .\scripts\windows-w2e\collect-cli.ps1 -Phase lifecycle -ProjectPath "<project>"
   ```

7. Leave Account A’s runtime running. On **Account B**:

   ```powershell
   .\scripts\windows-w2e\pipe-probe.ps1 -PipePath "\\.\pipe\moraine.capture.v1.<scopeId>"
   ```

   Use the redacted scope id / pipe path recorded under Account A (never the
   full SID). Expect access denied while the pipe exists.

8. Finish uninstall & ledger checks under Account A; then:

   ```powershell
   .\scripts\windows-w2e\collect-cli.ps1 -Phase uninstall -ProjectPath "<project>"
   .\scripts\windows-w2e\write-summary.ps1
   ```

9. Copy sanitized artifacts into the repo evidence package and fill
   `docs/evidence/W2E_WINDOWS_11_STANDARD_USER.md`.

10. Only after **every** mandatory gate passes, promote README / ARCHITECTURE /
    ROADMAP / DEVELOPMENT_BLUEPRINT support claims.

## Evidence output root

By default scripts write to:

```text
$env:LOCALAPPDATA\Moraine\w2e-evidence\<timestamp>\
```

Copy only sanitized files into:

```text
docs/evidence/windows-w2e/
```

## Staging location

Acceptance stages into:

```text
%LOCALAPPDATA%\Programs\Moraine\
```

That path is **not** the code default prefix (`%LOCALAPPDATA%\Moraine`). Session
scripts set `MORAINE_PREFIX` and prepend PATH so discovery matches the staged
suite. This is acceptance staging, not a supported installer.

## Defects

If a product gate fails:

1. Keep the local evidence directory.
2. Open a fix PR from current `main` (not a binary patch on the VM).
3. Restart W2-E from a clean snapshot against the new merge commit.

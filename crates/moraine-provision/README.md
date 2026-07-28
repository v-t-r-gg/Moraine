# moraine-provision

Shared installation-state inspection and onboarding for Moraine.

Both the CLI and the Tauri desktop call this crate. The desktop never scrapes
CLI stdout for setup/repair.

## API surface

- `inspect()` → `SystemState`
- `plan(intent)` → `SetupPlan`
- `apply(plan)` → `ApplyOutcome` (journaled; Ready, DirectVerified,
  RolledBack, or RollbackRequired)
- `rollback(receipt)` → restore backups / reverse completed ops
- `verify(intent)` → end-to-end capture self-test
- `health()` / `repair(action)` → structured doctor-class checks with Fix actions

## Traits

- `ServiceManager` — platform-abstracted background capture lifecycle
- `AgentAdapter` — detect / plan / apply / verify / remove for integrations (Codex first)

Service mutations use write-ahead prestate and attempt markers. Project-local
ledgers are intentionally retained during rollback and reported as degraded
retained state. ProductCapture verification proves the real hook/service/run
path and removes its uniquely bound synthetic run before reporting Ready.
New project-discovery registrations are also retained after a later setup
failure so an existing ledger does not disappear; the receipt reports that
retained rebuildable metadata explicitly.

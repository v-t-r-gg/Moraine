# moraine-provision

Shared installation-state inspection and onboarding for Moraine.

Both the CLI and the Tauri desktop call this crate. The desktop never scrapes
CLI stdout for setup/repair.

## API surface

- `inspect()` → `SystemState`
- `plan(intent)` → `SetupPlan`
- `apply(plan)` → `SetupReceipt` (journaled, reverse-order rollback)
- `rollback(receipt)` → restore backups / reverse completed ops
- `verify(intent)` → end-to-end capture self-test
- `health()` / `repair(action)` → structured doctor-class checks with Fix actions

## Traits

- `ServiceManager` — platform-abstracted background capture lifecycle
- `AgentAdapter` — detect / plan / apply / verify / remove for integrations (Codex first)

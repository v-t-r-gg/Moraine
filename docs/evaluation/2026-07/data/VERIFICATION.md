# Baseline verification results

**Commit:** `4f8d1e85011d8ea49d02ea537c45b29b579ce52b`  
**When:** 2026-07-21  
**Platform:** Arch Linux, rustc/cargo 1.97.1, node v26.4.0

| Command | Exit |
|---------|------|
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 |
| `cargo test --workspace` | 0 |
| `npm run typecheck` | 0 |
| `npm test` | 0 |
| `npm run build` | 0 |
| `./scripts/check.sh` | 0 |

Raw log: [baseline-checks.log](./baseline-checks.log)

**Not run in this capture:** `npm run tauri build -- --no-bundle` (time; previously green on PR #11 CI `tauri-check`).

**Flakes observed:** none in this run.

**Pre-existing:** none failing.

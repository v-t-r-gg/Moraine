# Compatibility and portability review

**Baseline:** `4f8d1e8`

## Support matrix

| Platform | Builds | Service model | Live tested (this eval) | Supported claim |
|----------|--------|---------------|-------------------------|-----------------|
| **Linux x86_64** | Yes (CI + local) | systemd --user unit helpers + manual binary | Yes (service, CLI, tests) | **Primary** (de facto) |
| **macOS** | Likely (Rust/Tauri) | launchd not shipped | No | **Not claimed** |
| **Windows** | Unproven | No Unix socket primary design | No | **Not claimed** |
| **Linux aarch64** | Unproven here | Same as x86_64 likely | No | **Not claimed** |

Do not advertise multi-platform support until live-tested.

## Assumptions that bind portability

| Assumption | Impact |
|------------|--------|
| Unix domain sockets for hooks | Windows needs alternate transport |
| `systemd --user` helpers | Linux-centric install docs |
| Desktop discovery `curl` | Requires curl on PATH |
| Tauri + WebKit | Platform package deps differ |
| Path canonicalization / symlinks | Linux tested via core dedupe; Windows case-insensitivity not tested |
| LF vs CRLF | Markdown/sidecar: needs care; tests mention CRLF for notes historically |
| File mode 0o700 on spool | Unix permissions |
| MSRV 1.88 | Enforced in CI |
| Node for frontend | CI/setup-node |

## Migration / downgrade

| Direction | Behavior |
|-----------|----------|
| Older → current (≤6) | Load path promotes schema where designed |
| Future schema → current | **Reject** (good) |
| Downgrade binary after new schema write | Unsupported; risk of unreadability — document as 1.0 policy |

## Portability verdict

**Beta:** Linux x86_64 only, stated explicitly.  
**1.0:** Either multi-platform with service model per OS, or explicit single-platform 1.0.

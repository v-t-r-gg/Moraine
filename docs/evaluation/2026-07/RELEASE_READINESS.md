# Release readiness

**Baseline:** `4f8d1e8`

## Release artifacts

| Artifact | Status |
|----------|--------|
| Versioned crates | 0.1.0 workspace (not meaningfully versioned for users) |
| GitHub Releases / installers | **None** |
| CLI publish path | `cargo install` risky (stale binary observed) |
| Desktop packages | Tauri build possible; no published bundles |
| Service package | Manual / systemd unit text |
| Changelog for users | Weak |
| License | Apache-2.0 present |
| SECURITY.md | **Missing** |
| CONTRIBUTING | **Missing** |
| Screenshots / demo video | Sparse |

## CI readiness

PR #11 merge head: rust, msrv, frontend, tauri-check **green**.  
Local baseline verification: fmt/clippy/test/typecheck/npm test/build/check.sh **green** (see [data/VERIFICATION.md](./data/VERIFICATION.md)).

## External beta checklist (current)

| Requirement | Met? |
|-------------|------|
| One OS documented as supported | Partial (implicit Linux) |
| One agent dogfood pack | No |
| Install produces protocol CLI | **No guarantee** |
| Capture without desktop | Designed; not stranger-proven |
| Discovery without path | Implemented |
| Redaction sealed all ordinary APIs | **No** on main |
| Threat model doc | No |
| Known limitations published | Partial in eval |

## Portfolio / demo readiness

Can demo **in development checkout** with built binaries.  
Cannot yet ship a clean “download and try” story for portfolio evaluation by strangers.

## Verdict

**Not release-ready for external beta.**  
Repository quality for developers is high; product packaging and trust seal incomplete.

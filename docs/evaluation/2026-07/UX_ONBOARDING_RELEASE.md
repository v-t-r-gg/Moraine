# Pass D — UX, onboarding, and release readiness

**Baseline:** `4f8d1e8`

## Journey map (intended)

```text
discover → install → start service → init project → configure agent
→ ordinary task → service failure → inspect run → finding → response
→ reopen later → upgrade
```

### Measured friction (this evaluation)

| Step | Reality | Undocumented / hard |
|------|---------|---------------------|
| Discover | README exists; vision clear | No short “60-second demo” |
| Install | `cargo`/npm/scripts; Arch setup script | **No release artifacts**; `cargo install` binary can be stale/incomplete |
| Start service | `moraine-service` + systemd unit helpers | Spool paths, socket location, loopback HTTP |
| Init project | `moraine project init` | Requires **current** CLI |
| Configure agent | `docs/integrations/CODEX.md` | Multi-file config; not automated |
| Ordinary task | Depends on Codex hooks + MCP | Not live-validated end-to-end here |
| Service failure | Spool designed | User-facing recovery messaging weak |
| Inspect run | M5 workspace helps | Still desktop build complexity (WebKit) |
| Finding/response | Desktop + MCP | Works in tests |
| Reopen later | Files durable | Discovery depends on scan/index |
| Upgrade | Schema promote on load | No migration guide for users |

## Documentation audit

| Doc | State |
|-----|--------|
| README | Good product framing; workflow still partially “Human notes/Save” residual |
| VISION | Coherent invariant |
| ARCHITECTURE | Updated for discovery |
| ROADMAP | **Stale relative to merged M5** (still lists M5 as “Now”) |
| BLUEPRINT aligned | Long; mixed current/future |
| MCP.md | Present |
| CODEX.md | Present; heavy |
| AGENT_RUN_PROTOCOL | Strong |
| DEVELOPMENT | Process notes |
| SECURITY.md | **Missing** |
| CONTRIBUTING | **Missing** |
| CHANGELOG | Weak/absent for users |
| Screenshots | Minimal |
| Installer docs | Dev-oriented only |

## Desktop UX

**Strengths**

- Default ledger workspace (projects → runs → detail)
- Offline/direct-scan messaging
- Append-only protocol panels
- Legacy mode labeling

**Weaknesses**

- Monolithic `App.tsx` still large despite workspace split
- Full collab editor still present
- Discovery uses curl probe
- Limited empty-state polish for real multi-project homes
- No proven cold-start onboarding wizard

## CLI UX

- Help text for current build is good (`project`/`run`/`mcp`/`hook-codex`)
- **Installed binary drift** is a release-blocking UX failure
- `decide` still listed (legacy) — OK if clearly marked

## Release / portfolio readiness

| Item | Ready? |
|------|--------|
| Versioning story | Weak (0.1.0 everywhere) |
| Platform matrix | Linux-first; others unproven |
| CI | Solid for repo checks |
| Packaging | No |
| External beta checklist | Incomplete |
| Support story | Single maintainer assumption |

## Scenario 5 (cold install) — honesty

**Not completed in a clean VM.** Evaluation used the development checkout.  
Cold-install conclusion is **inferred from packaging absence + binary drift**, not measured on a blank machine.

## Scenario 1 (full agent happy path) — honesty

**Not completed with a live Codex session** in this evaluation window.  
Service loopback discovery and protocol CLI dogfood **were** exercised. Automated tests cover large parts of the capture stack.

## UX verdict

Moraine is **developer-usable in-tree**, **not stranger-usable as a product**. M5 improved the inspection surface. Onboarding and packaging remain the dominant external beta blockers after integrity redaction.

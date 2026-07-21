# UX and onboarding review

**Baseline:** `4f8d1e8`  
Companion narrative: [UX_ONBOARDING_RELEASE.md](./UX_ONBOARDING_RELEASE.md)

## Journey friction (external user)

| Step | Commands / surfaces (approx) | Friction |
|------|------------------------------|----------|
| Discover | README | OK |
| Install | rustup, cargo build/install, npm, WebKit | High — no release artifact |
| Start service | `moraine-service` + spool/socket/http flags or systemd | High |
| Init project | `moraine project init` | Medium — needs **current** binary |
| Configure agent | Codex config + MCP + hooks (multi-file) | High |
| Ordinary task | Agent session | Unknown to stranger |
| Service failure | Spool files | Medium — weak user messaging |
| Inspect run | Desktop workspace | Medium after M5 |
| Challenge claim | Finding UI / MCP | Medium |
| Agent responds | MCP respond | Medium |
| Reopen later | Discovery | Medium |
| Upgrade | Schema promote on load | Low-med undocumented |

## Count estimates (first success path)

- **Commands:** ≥8 distinct (install toolchain, build/install, service start, project init, configure agent, agent run, open desktop, rebuild optional)
- **Config files:** ≥2–4 (Codex config, MCP entry, optional unit files)
- **Windows:** CLI + agent + desktop (+ optional terminal for service)
- **Source knowledge required:** Yes for WebKit deps, spool paths, binary freshness

## Documentation gaps for onboarding

- No SECURITY.md
- No CONTRIBUTING.md  
- Weak/missing user CHANGELOG
- ROADMAP still lists M5 as “Now” after merge
- Residual Human notes language vs observations
- Cold install not documented as measured path

## Desktop

**Strengths:** ledger workspace default; offline banner; filters; append-only panels; legacy labeling.  
**Weaknesses:** curl probe; large App shell; collab editor heritage; limited multi-project real-world polish.

## CLI

Current in-tree help includes `project`/`run`/`mcp`/`hook-codex`.  
**Stale `~/.cargo/bin/moraine` without those commands is an onboarding P1.**

## Verdict

**Developer-usable in-tree; not stranger-usable.** Beta needs C2 install + Codex pack after C1 redaction seal.

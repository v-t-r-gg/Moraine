# Live scenarios log

**Baseline:** `4f8d1e8`  
**Platform:** Linux, DISPLAY available, WebKit/Tauri buildable in prior sessions  

## Completed

### Service discovery (partial Scenario 2/4)

- Built `moraine-service`, bound loopback `127.0.0.1:33112`, Unix socket in temp spool.
- `GET /status` → online, revision present.
- `POST /index/rebuild` → revision 1→2, projectCount≥1.
- **Not done:** stop mid-hook stream with live Codex events; oversized/malformed live inject (covered by automated spool tests).

### Protocol dogfood (evaluation run)

- `./target/debug/moraine project init --json`
- `run start` evaluation objective → run `ce58b532-…`
- Checkpoint recorded for baseline verification.
- **Note:** `~/.cargo/bin/moraine` failed protocol subcommands (stale).

### Automated integrity stand-ins

- `cargo test -p moraine-core discovery` — 10 passed (nonmutation, redaction timeline, sequential amend).
- Findings redaction agent-path test **not on main** (lives on PR #12).
- Service discovery_index/http tests exist on main (from M5 merge).

### Desktop

- Prior M5 work: production binary launch (file watcher). **Not re-run full interactive multi-project UI** in this evaluation session.
- No new screenshots.

## Not completed (honest)

| Scenario | Status | Why |
|----------|--------|-----|
| 1 Full Codex happy path | **Not run** | No live Codex session in evaluation window |
| 2 Service interrupt + hook flood | **Partial** | Design/tests only; no live Codex hooks |
| 3 Full amend/supersede/redact + MCP leak audit | **Partial** | Core timeline tests; **finding leak remains on main** |
| 4 Move project unavailable | **Not run** | |
| 5 Cold install clean env | **Not run** | No clean VM |
| 6 Legacy collab share | **Not run** | Frozen/secondary |

## Implications

Beta claims **must not** include “validated full agent capture path in this evaluation.”  
Automated + partial live evidence supports **core/service trust**, not **stranger onboarding**.

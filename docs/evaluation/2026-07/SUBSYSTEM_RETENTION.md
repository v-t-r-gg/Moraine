# Subsystem retention review

Baseline: `4f8d1e8`

| Subsystem | Decision | Why |
|-----------|----------|-----|
| Agent-run protocol | **Keep** | Product center |
| Markdown projection | **Keep** | Hindsight + optional Git |
| Structured sidecar | **Keep** | Canonical structured ledger |
| Local service | **Keep / consolidate** | Required for capture-without-desktop; simplify install/debug |
| MCP server | **Keep** | Primary agent semantic channel |
| Codex adapter | **Keep / harden** | Only real integration; make path boring |
| Evidence system | **Keep / freeze feature expansion** | Needed for trust; no new capture types pre-beta |
| Findings | **Keep / consolidate redaction** | Core review loop; fix projections |
| Append-only operations | **Keep** | Integrity model |
| Redaction | **Keep / complete** | Merge PR #12 class fixes; single projection |
| Project/run discovery | **Keep / consolidate transport** | Desktop value; drop curl if possible later |
| React desktop | **Keep / thin** | Human surface; freeze collab growth |
| Tiptap | **Freeze** | Needed for legacy/collab; not protocol center |
| Yjs | **Freeze / defer expansion** | Secondary |
| WebSocket relay | **Freeze** | Optional demo; not beta path |
| Live sharing | **Freeze** | Nice; not beta |
| Web interface (dev) | **Freeze** | Dev aid |
| Annotations | **Freeze** | Parallel to findings; no expansion |
| Legacy free-form docs | **Deprecate slowly** | Label only; remove as default forever |
| Historical decisions | **Deprecate / freeze** | Read legacy; no new product investment |
| Collaboration history | **Freeze** | Local history OK; no multiplayer push |
| Version history | **Freeze** | Issue #4 open; no expansion |
| Direct FS fallback | **Keep** | Offline desktop honesty |
| Diagnostics HTTP | **Keep / constrain** | Loopback only; consider native client later |
| CLI surface | **Keep / ship current** | Must match protocol |

## Delete candidates (after beta packaging, not in this evaluation branch)

- Decision UX remnants in docs
- Excessive dual paths that confuse onboarding (document first, code later)
- Do **not** delete Yjs/share in panic before measuring dogfood need — **freeze** first

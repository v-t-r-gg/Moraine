# Capability matrix

**Baseline:** `4f8d1e8`  
Full narrative: [CAPABILITY_INVENTORY.md](./CAPABILITY_INVENTORY.md)

Columns: **I** implemented · **T** automated tests · **L** live-validated this evaluation · **D** documented · **U** external-user-ready

| Capability | I | T | L | D | U |
|------------|:-:|:-:|:-:|:-:|:-:|
| Workspace crates (core/cli/mcp/service/server/app) | ✓ | ✓ | partial | ✓ | partial |
| Agent run protocol CLI | ✓ | ✓ | ✓ | ✓ | partial |
| Project UUID identity | ✓ | ✓ | ✓ | ✓ | partial |
| Run start/checkpoint/ready/resume/show/open | ✓ | ✓ | ✓ | ✓ | partial |
| Local STDIO MCP | ✓ | ✓ | partial | ✓ | partial |
| Findings MCP (list/get/respond) | ✓ | ✓ | automated | ✓ | partial |
| Codex hook adapter | ✓ | ✓ | not full session | ✓ | no |
| Local service Unix socket | ✓ | ✓ | ✓ | partial | no |
| Spool + seen markers | ✓ | ✓ | partial | partial | no |
| Provisional runs | ✓ | ✓ | no | partial | no |
| Capture coverage | ✓ | ✓ | no | partial | no |
| Evidence capture | ✓ | ✓ | automated | ✓ | no |
| Secret scrubbing in evidence | ✓ | ✓ | automated | partial | no |
| Findings desktop | ✓ | ✓ | automated | partial | no |
| Append-only ops | ✓ | ✓ | automated | ✓ | no |
| Redaction ordinary timeline/UI | ✓ | ✓ | automated | partial | no |
| Redaction agent-facing findings | ✗ | PR#12 | no | no | no |
| Discovery core read models | ✓ | ✓ | automated | ✓ | no |
| Service discovery HTTP | ✓ | ✓ | ✓ | ✓ | no |
| Desktop discovery workspace | ✓ | ✓ | binary only | ✓ | no |
| Index rebuild nonmutation | ✓ | ✓ | ✓ | ✓ | no |
| React + Tauri desktop | ✓ | ✓ | partial | ✓ | no |
| Tiptap editor | ✓ | ✓ | no | partial | no |
| Yjs + relay | ✓ | partial | no | partial | no |
| Share/join | ✓ | ✓ | no | ✓ | no |
| Annotations | ✓ | ✓ | no | partial | no |
| Local edit history | ✓ | ✓ | no | partial | no |
| Legacy decide | ✓ | partial | no | ✓ | n/a |
| Schema migrate ≤6 / reject future | ✓ | ✓ | automated | partial | no |
| CI pipeline | ✓ | ✓ | CI green #11 | ✓ | n/a |
| Packaging/installers | ✗ | ✗ | ✗ | weak | **no** |
| Second agent | ✗ | ✗ | ✗ | aspirational | **no** |
| Cold install | weak | ✗ | ✗ | incomplete | **no** |

Machine-readable: [data/capability-matrix.json](./data/capability-matrix.json)

# Welcome to Moraine

**Moraine** is a local-first, Git-native collaborative Markdown editor —
think Google Docs, but for plain `.md` files that live in *your* folders
and repos.

## Phase 0–1 MVP

This build includes:

- **Tauri 2** desktop shell (Linux-first)
- **ProseMirror / Tiptap** rich Markdown editing
- **File open / save** with auto-save
- **Filesystem watcher** (reload when the file changes on disk)
- **Yjs** local multi-tab collaboration simulation
- **Edit history** snapshots (SQLite/Git come later)
- **CLI**: `moraine cat`, `edit`, `write`, `history`, `watch`

## Quick start

```bash
# CLI
cargo run -p moraine-cli -- info
cargo run -p moraine-cli -- cat examples/welcome.md

# Desktop (requires Linux WebKit deps)
npm install
npm run tauri:dev
```

## Roadmap

1. Real-time collab over self-hosted Axum + WebSockets  
2. Comments + suggestion mode  
3. Native Git integration  
4. Agent hooks (MCP / Ollama)  
5. Optional Docker server mode  

---

*Edit this file with Moraine — it is just Markdown.*

# Current-product demo (C3)

Short path for a **current** Moraine beta, not historical editor/share demos.

## Prerequisites

- Installed suite (`docs/INSTALL.md`) or contributor build with service running  
- Optional: Codex CLI for mechanical capture  

## Script (≈5 minutes)

1. **Install / start**
   ```bash
   moraine setup
   moraine service start
   moraine doctor
   ```
2. **Project**
   ```bash
   cd /tmp/moraine-demo-project && git init
   moraine project init .
   moraine integrate codex --project .   # optional
   ```
3. **Capture (desktop closed)**  
   Run a normal agent task, or synthetic hooks:
   ```bash
   printf '%s\n' '{"hook_event_name":"SessionStart","session_id":"demo-1","cwd":"'"$(pwd)"'","source":"startup"}' \
     | moraine hook-codex
   printf '%s\n' '{"hook_event_name":"UserPromptSubmit","session_id":"demo-1","cwd":"'"$(pwd)"'","prompt":"Demo task"}' \
     | moraine hook-codex
   sleep 3
   ls .moraine/runs/
   ```
4. **Inspect**  
   Launch installed desktop (`moraine open` or app menu).  
   **Projects → Runs → Ledger** — open the provisional/semantic run.  
   Screenshot placeholder: `docs/screenshots/ledger-workspace.png`.
5. **Offline**  
   `moraine service stop` → desktop still lists known paths via direct discovery when configured; doctor warns offline.
6. **Uninstall product (keep ledger)**
   ```bash
   ./uninstall.sh   # from release bundle
   test -d .moraine && echo ledger retained
   ```

## Screenshots

| File | Subject |
|------|---------|
| `docs/screenshots/ledger-workspace.png` | Projects → Runs → Ledger |
| `docs/screenshots/service-health.png` | Service health banner offline/online |

Generate on a graphical session after `npm run tauri:dev` or installed `moraine-app`. Until then, placeholders may be missing — do not invent product screenshots.

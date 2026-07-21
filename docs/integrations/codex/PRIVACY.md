# Privacy notes (Codex pack)

## What Moraine stores

- Project-local ledgers under `<project>/.moraine/` (runs, sessions, evidence sidecars).
- Optional spool/index under `~/.cache/moraine-service` (hook fallback and discovery index).
- Project-local Codex config under `<project>/.codex/` (commands and args only; no secrets written by Moraine).

## What Moraine does not do (C2)

- No remote sync or hosted upload.
- No authenticated identity.
- No automatic Codex installation or credential management.
- No secret harvesting into integration configs.

## Git

Teams choose whether to commit:

- `.codex/config.toml` / `hooks.json` (paths are absolute and machine-specific — often gitignored)
- `.moraine/` ledgers (team policy)

Integration backups (`.bak.<timestamp>`) may contain prior config text; treat as sensitive if the original was.

## Local attacker model

A local filesystem owner can read user-owned Moraine data. Permissions aim for user-only spool/socket/executables, not multi-user isolation.

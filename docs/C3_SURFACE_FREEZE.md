# C3 — Beta surface freeze

## Default product shell

```text
App
├── installed-product bootstrap (version, doctor hint)
├── service / discovery health banner
├── ledger workspace (primary)
│   ├── projects
│   ├── runs
│   ├── timeline
│   ├── findings
│   └── append-only actions
└── legacy document route (secondary, explicit open only)
```

## Frozen for beta (not removed from tree yet)

| Surface | Status |
|---------|--------|
| Live collab / Yjs relay | **Frozen** — no remote `syncUrl`; not in main coordinator |
| Welcome Markdown / share-first onboarding | **Frozen** |
| Free-form Human notes as protocol path | **Frozen** (legacy document only) |
| `moraine decide` product center | Already legacy |
| Second agent adapter | Deferred (after W1–W3 sequence) |

## Explicit non-goals this milestone

- `moraine-core::prelude` reorg  
- Broad evidence expansion  
- Semantic/vector search  
- Relay authentication  
- Richer Git/PR integration  
- Windows/macOS install (W1–W3)  

## Flags

See `src/app/surfaceFreeze.ts`.

# External beta review workspace

Moraine’s desktop review workspace is for people who need to inspect agent
runs without knowing internal schemas.

## Flow

```text
Projects → Runs → Review workspace
```

1. Add or select a project (local folder only; nothing is uploaded).
2. Find a run by objective, filters, or search.
3. Review Overview, Checkpoints, Evidence, Findings, and History.

## Product language

| Phrase | Meaning |
|---|---|
| Ready for review | Agent lifecycle state only — not approved or complete |
| Mechanical + semantic observed | Both primary capture channels landed — not full knowledge |
| Observed / Not observed | Durable facts for this run |
| Not supported by this adapter | Capability profile (e.g. Claude tool activity) |
| Open / Addressed / Archived finding | Descriptive review state (append-only) |

Moraine records review activity. It does not approve merges, deployments, or
releases.

## Evidence provenance

Keep these distinct:

* **Invocation observed** / **Result observed** / **Moraine captured** — Moraine’s mechanical observations
* **Agent reported** — a claim from the agent, not independently verified

## Capture fidelity

Capability answers what an adapter can emit. Observation answers what this run
recorded. Gaps are factual, not scores. Bound-session schema failures surface
explicitly (`captureFidelityError` / command error).

## Fixture and acceptance

```bash
# Disposable multi-run project via public CLI/hooks (Linux):
./scripts/create-review-workspace-fixture.sh --preserve

# Local review acceptance (frontend + Tauri command boundary + fixture):
./scripts/desktop-review-acceptance.sh
```

Generated project ledgers are never committed.

## Known beta limitations

* W2-E Windows interactive acceptance remains `not_executed`.
* Windows Product Ready remains **No**.
* No installer/signing/WinGet (W3).
* No approval or deployment controls.
* Graphical desktop automation uses compiled app tests + xvfb where available;
  full WebKit GUI driving is not required for this gate.

## Privacy

Prompt bodies and assistant transcripts are not stored by default. Redacted
targets must not reappear in ordinary review views.

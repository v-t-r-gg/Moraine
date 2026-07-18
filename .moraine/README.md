# Moraine project run records

This directory holds **development run records** for work on Moraine itself.

Each significant pull request should update (or create) a Markdown run under `runs/`. Commit both the `.md` file and its `*.md.moraine.json` sidecar when decisions are recorded.

## Layout

```text
.moraine/
  README.md
  runs/
    YYYY-MM-DD-<short-topic>.md
    YYYY-MM-DD-<short-topic>.md.moraine.json
```

## Workflow

1. Start or continue a run record for the issue or PR.
2. Update **Actions taken**, **Decisions and rationale**, and **Tests and evidence** while working.
3. At the end, fill **Review candidate** with the implementation commit and validation results.
4. A human reviews the run record (and code), then records a run-level decision in Moraine against the **finalized Markdown**.
5. Commit the updated ledger sidecar (sidecar-only commits do not require another decision).
6. Do not merge while the Moraine decision is stale relative to the run record. If implementation changes after approval, update the reviewed commit in the run record and record a new decision.

## Guarantee boundary

Moraine enforces that a human decision applies to the exact Markdown revision of the run record. It does **not** yet cryptographically bind that decision to a Git commit or source tree.

| Guarantee | Current status |
| --------- | -------------- |
| Decision applies to exact run-record Markdown | Mechanically enforced |
| Run record names an implementation commit | Manually recorded |
| Implementation commit has not changed | Process-enforced |
| Decision cryptographically applies to source tree | Not implemented |

## Conventions

* One primary run record per PR when practical.
* Name the reviewed implementation commit(s) in the Markdown body.
* After approval, do not change source, docs, or the run-record Markdown without re-review.
* Reviewer labels are attribution only, not authenticated identity.
* Do not claim compliance-grade or tamper-proof audit properties.

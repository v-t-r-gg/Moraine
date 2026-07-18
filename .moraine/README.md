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
4. A human reviews the run record (and code), then records a run-level decision in Moraine.
5. Commit the updated ledger sidecar.
6. Do not merge the PR while the decision is **stale** relative to the listed implementation commit.

## Conventions

* One primary run record per PR when practical.
* Link the decision to source commits in the Markdown body until Git evidence is a product feature.
* Reviewer labels are attribution only, not authenticated identity.
* Do not claim compliance-grade or tamper-proof audit properties.

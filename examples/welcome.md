# Agent run: welcome path check

> **Historical / compatibility sample.** Prefer the current product path:
> installed suite → project init → agent/hooks capture → desktop **Projects → Runs → Ledger**.
> Free-form Comment/Suggest on a Markdown path is secondary to the structured ledger workflow.

**Status:** complete (example record)  
**Agent:** example  
**Human review:** for protocol runs use the ledger workspace; this file is a legacy-style sample

## Objective

Smoke-check that Moraine can open a Markdown run record and support human review.

## Context

Local Moraine checkout. No production systems involved.

## Actions taken

1. Created this example run record under `examples/`.
2. Documented agent-run ledger positioning (see root README for install-first flow).
3. Did not modify application runtime code for this documentation pass.

## Decisions

* Treat this file as a **sample / historical** run record, not the primary install workflow.
* Prefer structured evidence and append-only observations on protocol runs; mechanical capture exists for supported hooks.

## Outcome

Documentation positioning updated toward agent-run review. No code behavior change.

## Evidence / verification

* Commands available: `moraine info`, `status`, `share`, `join`, `cat`, `write`, `edit`, `history`, `restore`, `watch` (see `moraine --help`).
* `./scripts/check.sh` used to validate the tree after doc edits.

## Risks

* Example may drift if CLI flags change; re-check against `moraine --help`.

## Open questions for human review

* Is the run-record template shape useful for real agent wrappers?
* Which fields should become structured metadata later vs stay freeform Markdown?

## Needs human input

* Confirm product wording: "review ledger" vs shorter alternatives for the README lead.

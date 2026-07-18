# Agent run: welcome path check

**Status:** complete (example record)  
**Agent:** example  
**Human review:** open this file in Moraine; try Comment / Suggest on a selection

## Objective

Smoke-check that Moraine can open a Markdown run record and support human review.

## Context

Local Moraine checkout. No production systems involved.

## Actions taken

1. Created this example run record under `examples/`.
2. Documented CLI share/status usage in the project README.
3. Did not modify application runtime code for this documentation pass.

## Decisions

* Treat this file as a **sample run record**, not a generic shared note.
* Keep evidence as manual links/notes only (no automatic capture in Moraine today).

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

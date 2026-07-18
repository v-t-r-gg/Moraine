# Development process

Short process notes for keeping `main` releasable. Product vision lives in [VISION.md](../VISION.md).

## Branch model

* **`main`**: releasable; no long multi-milestone work.
* **`release/*`**: stabilize and review a cut before merge to `main`.
* **`milestone/*`**: one integration branch per milestone (for example `milestone/v0.3-durable-annotations`).
* Optional short `feature/*` branches merge into the milestone branch when a slice is large.

Prefer several logical commits on the milestone branch over one giant agent commit.

## Release gate

1. Implement on a feature or release branch (never push multi-commit work only as direct `main` history without review when avoidable).
2. Open a pull request into `main`.
3. CI must run `./scripts/check.sh` (see `.github/workflows/ci.yml`).
4. Update the Moraine run record under `.moraine/runs/` with validation results and the reviewed implementation commit(s).
5. Human reviews code and run record; records a **current** run-level decision against the finalized run-record Markdown.
6. Commit the decision sidecar only. A sidecar-only approval commit does not require another decision.
7. Merge with a **merge commit** when preserving reviewed commit identities matters (for example when local `main` already contains some of the same commits). Prefer squash only when history rewrite is intentional.
8. Tag only when package versions match the product milestone, otherwise use a descriptive milestone tag (for example `review-ledger-v0.2.1`).

### What Moraine enforces vs process

| Guarantee | Current status |
| --------- | -------------- |
| Decision applies to exact run-record Markdown | Mechanically enforced |
| Run record names an implementation commit | Manually recorded |
| Implementation commit has not changed | Process-enforced |
| Decision cryptographically applies to source tree | Not implemented |

Do not merge while the Moraine decision is stale relative to the run record. If implementation changes after approval, update the reviewed commit in the run record and record a new decision.

## Recommended branch protection (`main`)

Configure in GitHub settings (requires admin):

* Require a pull request before merging.
* Require status checks to pass (Rust, frontend, Tauri / `./scripts/check.sh`).
* Require branch to be up to date with `main`.
* Block force pushes.
* Block deletion of `main`.
* Do not require a separate human reviewer on a solo repository unless another reviewer is available.

Self-approval is fine; the goal is a review boundary, not bureaucracy.

## After each milestone

Dogfood for several real runs before starting the next major milestone. Classify findings:

| Classification | Action |
|----------------|--------|
| Data loss or incorrect review state | Fix before next milestone |
| Workflow blocker | Fix before next milestone |
| Frequent usability issue | Consider patch release |
| Cosmetic | Backlog |
| New capability | Roadmap |
| One-off preference | Do not build immediately |

## Next milestone (detail only the next one)

**v0.3 Durable Annotations**: comments and suggestions survive ordinary document evolution; no full-list last-writer-wins annotation updates; orphan/ambiguous handling.

Later (adjustable): v0.4 Evidence References, v0.5 Agent Capture, v0.6 Review Inbox, v0.7 Collaboration Hardening.

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
4. Update the Moraine run record under `.moraine/runs/`.
5. Human reviews code and run record; records a **current** run-level decision.
6. Merge (squash or carefully structured merge).
7. Tag only when package versions match the product milestone, otherwise use a descriptive milestone tag (for example `review-ledger-v0.2.1`).

## Recommended branch protection (`main`)

Configure in GitHub settings (requires admin):

* Require a pull request before merging.
* Require status checks to pass (CI / `./scripts/check.sh`).
* Require branch to be up to date with `main`.
* Block force pushes.
* Block deletion of `main`.
* Prefer squash merges for solo work, or regular merges when preserving intentional commit history.

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

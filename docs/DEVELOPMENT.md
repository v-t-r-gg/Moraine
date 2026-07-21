# Development process

Contributor process and **source-tree** workflows. End users install from a release bundle—see [INSTALL.md](./INSTALL.md) and the top-level [README.md](../README.md). Do not present `cargo run` / `npm run dev` as the normal user path.

Product vision: [VISION.md](../VISION.md). Canonical blueprint: [DEVELOPMENT_BLUEPRINT_ALIGNED.md](./DEVELOPMENT_BLUEPRINT_ALIGNED.md) (stub: [DEVELOPMENT_BLUEPRINT.md](./DEVELOPMENT_BLUEPRINT.md)).

Desktop UI is **React + TypeScript + Vite** (Tauri host).

## Contributor setup (source checkout)

```bash
# Arch example
./scripts/setup-arch.sh   # rust, node; webkit for desktop
npm install

# CLI / service (no WebKit required)
cargo build -p moraine-cli -p moraine-service
cargo run -p moraine-cli -- version --json

# Desktop (dev)
npm run typecheck
npm test
npm run build
npm run tauri:dev
# optional: MORAINE_OPEN=/absolute/path/to/run.md npm run tauri:dev

# Optional legacy live relay (in-memory; local only; not the primary product path)
cargo run -p moraine-server
cargo run -p moraine-cli -- share path/to/file.md --json
```

**Rust MSRV:** workspace `rust-version` (currently `1.88`). CI includes an MSRV job.

### Frontend scripts

| Script | Purpose |
|--------|---------|
| `npm run typecheck` | `tsc --noEmit` |
| `npm test` | Vitest |
| `npm run build` | Production web assets for Tauri |
| `npm run tauri:dev` | Dev desktop host |
| `npm run tauri build -- --no-bundle` | Compile app without packaging |

### Release bundle (developers)

```bash
./scripts/build-linux-release.sh
# → dist/moraine-<version>-linux-x86_64.tar.gz
```

Requires the Rust toolchain (and Node for desktop packaging). The **artifact** installs without Rust/Node.

### Checks

```bash
./scripts/check.sh
cargo fmt --all -- --check
cargo clippy -p moraine-core -p moraine-cli -p moraine-mcp -p moraine-server -p moraine-service -- -D warnings
```

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
4. Update the Moraine run record under `.moraine/runs/` with validation results, meaningful checkpoints, evidence, risks, and unresolved questions.
5. Human inspects code and run record (comments, notes, challenges as needed). **No Moraine approval/decision is required** as a merge gate.
6. Merge with a **merge commit** when preserving reviewed commit identities matters (for example when local `main` already contains some of the same commits). Prefer squash only when history rewrite is intentional.
7. Tag only when package versions match the product milestone, otherwise use a descriptive milestone tag.

### What Moraine enforces vs process

| Guarantee | Current status |
| --------- | -------------- |
| Run record is durable beside the work | Product behavior |
| Run record names an implementation commit | Manually recorded when useful |
| Implementation commit has not changed | Process / Git / PR review |
| Moraine authorizes merge or deployment | **Not a product goal** |

GitHub pull requests and CI remain responsible for merge workflow. Moraine preserves the work record and human context around it.

### Run records and Git SHAs

A committed run record must **not** attempt to contain the SHA of the commit that contains that same record. Record implementation commits in the run record when useful; record the final PR head in pull-request metadata when needed for GitHub review.

## Recommended branch protection (`main`)

Configure in GitHub settings (requires admin):

* Require a pull request before merging.
* Require status checks to pass (Rust, frontend, Tauri / `./scripts/check.sh`).
* Require branch to be up to date with `main`.
* Block force pushes.
* Block deletion of `main`.
* Do not require a separate human reviewer on a solo repository unless another reviewer is available.

Self-merge is fine when checks pass; the goal is a review boundary in GitHub/CI, not a Moraine verdict.

## Definition of done (dogfood runs)

A milestone or feature PR is not done while its Moraine run remains stuck mid-checkpoint.

Before calling a change set ready for human inspection / merge consideration:

1. Meaningful checkpoints cover the actual work (typically 3–8).
2. Validation evidence is linked or noted (commands, CI, dogfood findings).
3. Open risks and questions are current.
4. Lifecycle is `ready_for_review` (or an explicit later descriptive state when those exist).
5. CI includes every new crate on the critical path (for example `moraine-mcp` must be clippy’d and tested).

`ready_for_review` means ready for inspection—not approval.

## After each milestone

Dogfood for several real runs before starting the next major milestone. Classify findings:

| Classification | Action |
|----------------|--------|
| Data loss or incorrect ledger state | Fix before next milestone |
| Workflow blocker | Fix before next milestone |
| Frequent usability issue | Consider patch release |
| Cosmetic | Backlog |
| New capability | Roadmap |
| One-off preference | Do not build immediately |

## Milestone status

| Milestone | Status |
|-----------|--------|
| M4.5 React migration | complete |
| M4.6 append-only ledger semantics | complete |
| M5 local run discovery and ledger-focused UX | complete |
| C1 redaction ordinary projections | complete |
| C2 stranger-safe Linux install + Codex pack | candidate (see ROADMAP) |
| C3 beta hardening / surface freeze | next |

See [ROADMAP.md](../ROADMAP.md) and [DEVELOPMENT_BLUEPRINT_ALIGNED.md](./DEVELOPMENT_BLUEPRINT_ALIGNED.md).

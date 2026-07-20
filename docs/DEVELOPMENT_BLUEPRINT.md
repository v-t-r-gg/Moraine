# Moraine Development Blueprint

**Status:** Product and architecture direction for continued development  
**Repository:** `https://github.com/v-t-r-gg/Moraine`  
**Baseline:** Agent Run Protocol Foundation merged into `main`  
**Purpose:** Align Moraine around a durable ledger model, remove approval workflow from the product center, and define the path from the current protocol foundation to a credible MVP and portfolio-ready product.

This is the canonical blueprint copy under `docs/`.

---

## 1. Executive summary

Moraine is a **local-first ledger for autonomous agent work**.

Its primary object is an **agent run**: a bounded unit of work performed by an agent. Moraine preserves what the agent did, why it did it, what evidence exists, what risks or unresolved questions remain, and what human context accumulated around the work.

Moraine is **not** an approval system. It does not decide whether work is accepted, rejected, mergeable, deployable, or authorized. Those decisions normally remain in the coding-agent session, pull request, issue tracker, CI system, or other workflow that already owns them.

The product invariant is:

> **Moraine records review activity; it does not render the verdict.**

The near-term product should focus on coding agents:

> **Moraine is a local-first, cross-agent ledger for coding-agent work.**

The long-term architecture may support other autonomous agents, but the first marketable product should solve one narrow problem well: preserving a durable, source-adjacent work record that remains useful after the original agent session is gone.

---

## 2. Vision

### 2.1 Long-term vision

> Moraine is a local-first ledger for autonomous agent work.

Agents perform bounded runs. Moraine records their actions, implementation choices, outcomes, risks, unresolved questions, and evidence in durable files beside the work. Humans inspect those records, add comments and context, challenge claims, and preserve important review discussion.

The resulting artifacts remain readable and portable without requiring the original agent product, chat transcript, hosted service, or proprietary session viewer.

### 2.2 Near-term product position

> Moraine creates durable, source-adjacent records of coding-agent work that remain useful after the original agent session is gone.

Alternative concise positioning:

> **Review agent work without relying on agent chat.**

### 2.3 Core value proposition

Moraine should help a developer answer:

- What task did the agent perform?
- What meaningful actions did it take?
- Why did it choose this approach?
- What changed?
- What evidence supports its claims?
- What failed or remains uncertain?
- What concerns or clarifications did humans raise?
- How did the record evolve after feedback or resumed work?

Moraine should not answer:

- Was this work formally approved?
- May this pull request merge?
- May this deployment proceed?
- Did an authenticated authority authorize the work?

---

## 3. Product principles

### 3.1 Ledger, not workflow gate

Moraine records the work and the review context around it. It does not become another approval gate layered on top of the agent session, GitHub, CI, or deployment tooling.

### 3.2 Agent run, not document

The conceptual object is the run. Markdown is a durable representation of the run, not the product’s fundamental unit.

### 3.3 Local-first and source-adjacent

Run artifacts live beside the work and remain controlled by the user. They may be committed to Git, left local, archived, copied, or inspected with ordinary tools.

### 3.4 Tool independence

Moraine must not depend on one agent vendor. Codex, Claude Code, local agents, scripts, and future tools should use the same core run protocol.

### 3.5 Human-readable durability

A run must remain understandable through its Markdown record even when Moraine is not installed. Structured state may support safe continuation and rendering, but the human-readable artifact must not become an empty shell.

### 3.6 Sparse semantic capture

Agents should record only review-relevant meaning. Moraine should mechanically capture facts that do not require model judgment.

### 3.7 Honest trust boundaries

Agent-authored statements are claims. Agent-reported tests are not equivalent to Moraine-captured command results. Linked evidence is not automatically verified. Moraine is not tamper-proof, authenticated, signed, or compliance-grade unless those properties are explicitly implemented later.

### 3.8 Review without verdict

Human review may involve reading, commenting, challenging evidence, requesting clarification, responding to risks, and adding context. It does not need to end in an approval state.

---

## 4. Initial market niche

### 4.1 First users

The first target users are:

- individual developers who regularly use coding agents;
- open-source maintainers reviewing agent-generated changes;
- small engineering teams using more than one coding agent;
- local-model and privacy-conscious developers;
- developers running long, interrupted, or concurrent agent tasks;
- developers who want durable context independent of a vendor’s session history.

### 4.2 Market gap

Moraine occupies the gap between:

- ephemeral or vendor-specific agent chats;
- Git history and pull requests, which show changes but not the full work rationale;
- full observability systems, which capture traces but do not provide a concise source-adjacent human review record.

Moraine’s differentiation is the combination of:

- local ownership;
- cross-agent compatibility;
- durable plain files;
- source-adjacent records;
- sparse semantic checkpoints;
- evidence provenance;
- human comments and context;
- portability beyond the originating agent session.

### 4.3 What Moraine is not competing with

Moraine should not become:

- a full agent observability platform;
- a trace viewer;
- a token-cost dashboard;
- a prompt-management system;
- an agent orchestrator;
- a pull-request review replacement;
- a CI or deployment approval system;
- a general Markdown knowledge workspace;
- an enterprise governance platform in the near term.

Moraine may link to those systems as evidence providers or external workflow owners.

---

## 5. Canonical product model

### 5.1 Agent run

A bounded unit of work performed by an agent.

Examples:

- implement one feature;
- investigate one defect;
- perform one migration attempt;
- review one subsystem;
- update one documentation area;
- diagnose one production problem.

A long interactive agent session may contain multiple runs. A run is not automatically identical to a process, terminal session, or chat thread.

### 5.2 Run bundle

The complete durable object is the **run bundle**:

```text
Run bundle
├── Structured run state and operations
├── Human-readable Markdown projection
├── Human-controlled notes
├── Comments and review findings
├── Evidence references or captured evidence
└── Sidecar identity, revisions, and compatibility metadata
```

### 5.3 Run record

The run record is the human-readable Markdown representation of the run bundle.

It should include:

- objective;
- lifecycle state;
- starting project and Git context;
- chronological checkpoints;
- actions;
- rationale;
- evidence;
- risks;
- unresolved questions;
- human notes.

### 5.4 Structured sidecar

The sidecar stores machine-managed state required for:

- stable run identity;
- structured checkpoints;
- safe targeted mutation;
- idempotency;
- revision preconditions;
- recovery;
- lifecycle;
- annotations and findings;
- evidence metadata;
- schema compatibility.

### 5.5 Checkpoint

A sparse, meaningful record of a development boundary.

A checkpoint may contain:

- concise summary;
- actions completed;
- implementation rationale;
- evidence references;
- risks;
- open questions;
- mechanically captured Git context.

A normal development run should generally require approximately three to eight checkpoints, not a diary entry for every tool call.

### 5.6 Evidence

Evidence is information supporting or contextualizing a run claim.

Initial provenance categories:

- `agent_reported`: supplied by the agent without independent capture;
- `moraine_captured`: captured directly by a trusted Moraine operation;
- `external_reference`: link or path to an external system or artifact;
- `human_context`: contextual evidence or note supplied by a human.

Moraine must present provenance clearly and must never silently upgrade agent-reported evidence to captured evidence.

### 5.7 Human notes

Human notes are free-form human-controlled Markdown preserved exactly. They are contextual ledger content, not an approval mechanism.

### 5.8 Review finding

A review finding is a durable human observation attached to a run, checkpoint, rationale, risk, question, or evidence item.

Possible finding kinds:

- clarification requested;
- evidence requested;
- possible error;
- risk noted;
- context;
- follow-up.

Findings are informative and conversational. They do not authorize or block work.

### 5.9 Lifecycle

Lifecycle describes the run’s operational stage.

Current minimum states:

- `active`;
- `ready_for_review`.

Current transition:

- `resume` returns a ready run to active work while preserving history.

Possible later descriptive states:

- `completed`;
- `abandoned`.

Lifecycle is not approval.

---

## 6. Human interaction model

| Area | Human action |
|---|---|
| Objective and checkpoints | Read and comment |
| Rationale | Comment or request clarification |
| Evidence | Inspect, challenge, or request more |
| Risks and questions | Comment and respond |
| Human notes | Directly edit |
| Run lifecycle | Observe |

### 6.1 Managed-region authority

For protocol-created runs:

- structured sidecar state is canonical for machine-managed regions;
- Markdown managed regions are deterministic projections;
- managed regions are read-only in the desktop application;
- comments remain allowed on managed content;
- direct text replacement and suggestion acceptance must not rewrite managed content;
- only the Human notes region is free-form editable.

Legacy free-form Markdown records may retain their existing editing behavior.

### 6.2 Agent amendments

When a human identifies an inaccurate or incomplete statement, the agent should amend the ledger through a protocol operation. Historical claims should not be silently rewritten without an identifiable operation.

A future amendment model should preserve:

- original checkpoint identity;
- human finding;
- agent response;
- amendment timestamp;
- superseded or clarified relationship.

---

## 7. Agent interaction model

### 7.1 Normal experience

After one-time project and agent integration, a user should give the agent an ordinary task with no Moraine-specific prompt text.

The agent should automatically:

1. start a run before substantive work;
2. retain the returned run ID and revision;
3. record sparse checkpoints;
4. attach or reference evidence;
5. mark the run ready after validation;
6. resume the run when additional work occurs.

The user should not manually:

- create directories;
- choose run filenames;
- write templates;
- initialize sidecars;
- pass paths into prompts;
- ask the agent to reread the full Markdown;
- relay routine ledger instructions every task.

### 7.2 Agent protocol surface

The current and near-term protocol should center on:

- `run_start`;
- `run_show`;
- `run_checkpoint`;
- `run_ready`;
- `run_resume`.

Possible later additions:

- `run_complete`;
- `run_abandon`;
- `finding_list`;
- `finding_respond`;
- `run_amend`;
- evidence-capture operations.

No agent transport may expose approval, rejection, merge authority, reviewer identity, or workflow authorization.

### 7.3 Token-efficiency requirements

Token overhead is a product requirement.

Normal operation should:

- avoid returning full Markdown after mutations;
- avoid resubmitting Git facts;
- avoid full-record reads;
- keep success responses compact;
- capture only meaningful semantic checkpoints;
- mechanically render readable Markdown;
- bound resume packets as runs grow.

Initial product target:

> Moraine-related frontier-model overhead should remain below five percent for a normal coding task.

This is a target to measure, not a current marketing claim.

---

## 8. Architecture

### 8.1 Target shape

```text
                         ┌── JSON CLI
Agent integrations ─────┼── local MCP transport
                         └── future adapters/hooks
                                  │
                            moraine-core
                                  │
              ┌───────────────────┼───────────────────┐
              │                   │                   │
       structured sidecar   Markdown projection   evidence files
              │                   │                   │
              └───────────────────┼───────────────────┘
                                  │
                         desktop human UI
```

### 8.2 Core rules

Business logic belongs in `moraine-core`.

The CLI, MCP server, Tauri commands, and future adapters must call the same core operations. They must not maintain separate persistence or lifecycle rules.

### 8.3 Operation-based mutation

All protocol mutations should be narrow logical operations rather than complete document replacement.

Required properties:

- stable operation ID;
- idempotency key;
- expected revision or content hash;
- durable per-record serialization;
- atomic individual-file replacement;
- explicit two-file recovery;
- structured conflicts;
- deterministic rendering.

### 8.4 Append-oriented semantics

The sidecar should conceptually retain an append-oriented history of important run operations:

- run started;
- checkpoint recorded;
- risk recorded;
- evidence linked;
- run marked ready;
- run resumed;
- finding added;
- finding answered;
- amendment recorded.

A materialized state may be retained for efficient reads and rendering. The event semantics should remain identifiable and replay-safe.

### 8.5 Plain-file durability

No central database is required for the local product.

Project discovery and run lists may scan `.moraine` project metadata and sidecars. Indexes may be generated locally as caches, but the source artifacts remain portable files.

### 8.6 Live collaboration

The existing Yjs/WebSocket collaboration capability is secondary.

Freeze broad live-collaboration investment unless real use demonstrates that it is necessary. Do not let relay, rooms, browser authority, or rich collaborative editing dominate the roadmap.

---

## 9. Existing run-decision capability

The current repository contains run-level decisions such as `approved`, `changes_requested`, and `rejected`. These are no longer part of the core product vision.

### 9.1 Compatibility policy

Do not delete historical decision data abruptly.

Near-term policy:

- preserve existing decision fields during schema reads and migrations;
- preserve old records exactly;
- stop extending decision functionality;
- remove decision language from the product headline, vision, primary workflow, and examples;
- remove or hide decision controls from the primary desktop experience;
- mark the `decide` CLI path as legacy or compatibility-only;
- do not expose decisions through MCP or future agent transports;
- do not require decisions in development-process gates for Moraine itself;
- do not create new product features around decision freshness.

A later compatibility milestone may remove new decision creation while retaining read-only display of historical records.

### 9.2 Replacement concept

Replace decision-centric review with:

- comments;
- findings;
- evidence challenges;
- human notes;
- agent responses;
- amendments;
- descriptive lifecycle.

Moraine records what reviewers observed and how the record changed. External systems retain responsibility for final disposition.

---

## 10. Minimum viable product

A credible MVP should support this workflow:

```text
1. Install Moraine
2. Initialize or connect a repository once
3. Configure a coding agent once
4. Give the agent an ordinary coding task
5. Moraine automatically starts a run
6. Agent records 3–8 sparse checkpoints
7. Mechanical Git context is included
8. Important command/test evidence is captured or linked
9. Human opens the run from a local run list
10. Human comments, challenges evidence, or adds notes
11. Agent can read findings and record responses or amendments
12. Durable Markdown + sidecar + evidence remain beside the project
```

### 10.1 MVP requirements

- stable agent-run protocol;
- local MCP transport;
- Codex integration;
- at least one second tested agent integration before public beta;
- zero Moraine-specific per-task prompt text;
- compact checkpoints;
- basic captured command/test evidence;
- project and run discovery;
- simple ready-run list;
- durable findings and human notes;
- agent-readable findings and amendment flow;
- straightforward installation;
- complete demo repository and walkthrough.

### 10.2 MVP non-requirements

- formal approval states;
- remote MCP;
- hosted multi-user service;
- authenticated identities;
- cryptographic signatures;
- full trace ingestion;
- agent orchestration;
- advanced history UI;
- live-collaboration hardening;
- compliance features;
- enterprise policy enforcement.

---

## 11. Development roadmap

### Milestone 0 — Vision realignment and decision de-centering

**Goal:** Make the repository accurately express the ledger-only product boundary.

Scope:

- update `README.md`, `VISION.md`, `ARCHITECTURE.md`, `ROADMAP.md`, package metadata, and repository About text;
- define review as inspection, comment, challenge, context, and response;
- remove approval language from the product headline and normal workflow;
- remove or hide run-decision controls from the primary desktop UI;
- mark `moraine decide` as legacy/compatibility-only;
- preserve existing decision data and migration compatibility;
- remove decision requirements from Moraine’s own PR and run-record development process;
- add this blueprint or an adapted version under `docs/`.

Acceptance criteria:

- a new reader does not understand Moraine as an approval system;
- existing sidecars remain readable;
- no agent protocol tool grants decision authority;
- current tests for historical decision loading remain intact;
- normal documentation examples contain no approval command.

### Milestone 1 — Local MCP transport and Codex integration

**Goal:** Eliminate manual run setup and per-task prompt ceremony.

Scope:

- STDIO MCP server over existing core operations;
- tools: start, show, checkpoint, ready, resume;
- fixed project confinement;
- concise server instructions;
- Codex project configuration documentation;
- no full Markdown in normal responses;
- no decision tool;
- lifecycle, idempotency, recovery, and concurrency tests through MCP;
- one real task started from an ordinary prompt.

Acceptance criteria:

- one-time integration only;
- normal coding prompt starts and maintains a run;
- no manual filename or sidecar handling;
- no Moraine-specific task prompt;
- useful record produced with bounded token overhead.

### Milestone 2 — Minimal evidence capture

**Goal:** Move important verification facts from agent claim to mechanically captured evidence.

Scope:

- bounded `moraine exec` or equivalent trusted capture operation;
- exact command;
- working directory;
- timestamps;
- exit code;
- selected output artifact;
- output hash;
- current Git head and changed-file summary;
- external URL/path evidence references;
- clear provenance rendering.

Non-goals:

- full terminal recording;
- full observability traces;
- prompt or model telemetry;
- arbitrary remote execution.

Acceptance criteria:

- a reviewer can distinguish agent-reported from captured evidence;
- captured evidence is linked into the run without large model payloads;
- failure output is preserved honestly;
- no command is claimed as captured unless Moraine executed or directly observed it.

### Milestone 3 — Findings and amendment loop

**Goal:** Let human review context flow durably between the desktop and agent without introducing verdicts.

Scope:

- typed review findings;
- findings attached to checkpoints, rationale, evidence, risks, or questions;
- open/addressed/archived descriptive state;
- MCP tools for listing and responding to findings;
- agent amendment operations;
- durable relationship between original claim, finding, response, and amendment;
- comments remain usable without a verdict.

Acceptance criteria:

- a human can challenge a claim in the desktop;
- the agent can read the finding through its transport;
- the agent can respond or amend the run;
- the ledger preserves the complete exchange;
- no approval or rejection state is introduced.

### Milestone 4 — Local run discovery and ledger UX

**Goal:** Make the desktop useful across multiple runs without a central database.

Scope:

- project list;
- run list derived from local sidecars;
- filters for active, ready, completed, abandoned, open findings, or unresolved questions;
- recent-run navigation;
- evidence and finding counts;
- clear read-only managed sections and editable Human notes;
- reduced emphasis on generic Markdown editing;
- no approval inbox.

Acceptance criteria:

- a user does not need to know a run path;
- runs remain discoverable after restart;
- project scanning does not mutate records;
- the UI emphasizes ledger inspection rather than document authoring.

### Milestone 5 — Second agent integration, packaging, and external beta

**Goal:** Prove vendor-neutral value and make installation reproducible.

Scope:

- second tested agent, preferably Claude Code or another MCP-capable tool;
- installation packages or documented installer;
- versioned configuration and migrations;
- polished demo repository;
- screenshots and short video walkthrough;
- five external developer testers;
- structured feedback and repeat-use measurement.

Acceptance criteria:

- at least two agents can create equivalent run bundles;
- setup is achievable without maintainer assistance;
- external users voluntarily use Moraine for another task;
- most normal runs are reviewable without reopening the full agent transcript.

---

## 12. Quality and trust requirements

### 12.1 Data integrity

Immediate blockers:

- data loss;
- ghost or duplicated checkpoints;
- wrong restored content;
- silent last-writer-wins loss;
- evidence provenance escalation;
- human notes overwritten;
- annotations or findings silently disappearing;
- unrecoverable incomplete operations.

### 12.2 Compatibility

- preserve existing v3/v4 sidecars;
- reject unsupported future schema versions;
- migrate deterministically;
- never delete historical decisions or annotations during ordinary migration;
- test LF/CRLF and exact-byte Human notes preservation;
- maintain read-only compatibility for legacy records.

### 12.3 Security

Near-term security model:

- local trusted-user environment;
- no network listener for MCP;
- project-confined filesystem access;
- no arbitrary path writes;
- no hidden command execution through ledger tools;
- no secret or full-transcript ingestion by default;
- no claim of authenticated actor identity.

### 12.4 Performance and token cost

Measure:

- bytes returned by tools;
- estimated model token overhead;
- checkpoint count;
- full-record reads;
- Markdown rendering time;
- project scan time;
- evidence artifact size.

Normal tool results should remain compact and growth-bounded.

---

## 13. Product success metrics

Initial targets:

| Metric | Target |
|---|---:|
| Initial project setup | Under 5 minutes |
| Moraine-specific text in normal task prompt | Zero |
| Frontier-model token overhead | Below 5% |
| Standard checkpoints per run | 3–8 |
| Full Markdown reads by agent | Normally zero |
| Runs understandable without transcript | At least 80% of normal tasks |
| Small-run human inspection time | Under 5 minutes |
| Data-integrity failures | Zero |
| Tested coding-agent integrations for beta | At least 2 |
| Repeat use by external testers | Majority voluntarily use again |

These are product targets and must not be presented as achieved until measured.

---

## 14. Portfolio showcase definition

Moraine is already technically substantial, but a flagship portfolio release should demonstrate the complete product loop.

A polished showcase should include:

- stable protocol and persistence architecture;
- working MCP integration with a real coding agent;
- one normal prompt producing a run automatically;
- captured test evidence;
- desktop run discovery;
- human comments/findings and agent amendments;
- durable Markdown and sidecar visible in the demo repository;
- clean installation instructions;
- architecture diagram;
- concise trust and limitation statement;
- screenshots and a short demonstration video;
- a real case study comparing the run record with the original agent transcript.

The current project is close to a strong protocol showcase but not yet a polished end-user product. The remaining distance is primarily productization, integration, evidence, and UX—not foundational persistence correctness.

A reasonable development horizon is approximately four to six bounded milestones or pull requests, not a broad rewrite.

---

## 15. Explicit non-goals

For the foreseeable MVP scope, Moraine is not:

- an approval or rejection system;
- a merge gate;
- an agent orchestrator;
- a full observability stack;
- a prompt or trace management platform;
- a CI replacement;
- a Git or pull-request replacement;
- a general-purpose Markdown editor;
- a knowledge-management workspace;
- a compliance-grade audit system;
- a cryptographically signed ledger;
- a hosted multi-tenant collaboration service;
- a generalized enterprise governance platform.

---

## 16. Development process

### 16.1 Bounded milestones

Each milestone should solve one observed product problem and avoid speculative adjacent work.

### 16.2 Dogfooding

Dogfood the interface being built, not obsolete manual scaffolding.

Examples:

- MCP work should be tested through MCP;
- evidence capture should be tested on real commands;
- findings should be tested through a human-to-agent exchange;
- run discovery should be tested with a real multi-run project.

### 16.3 Run records

Moraine development should continue using its own run protocol, but no human decision is required as a product or process gate.

A development run should retain:

- objective;
- scope;
- starting state;
- meaningful checkpoints;
- evidence;
- risks;
- unresolved questions;
- human findings and notes;
- final descriptive lifecycle state.

GitHub pull requests and CI remain responsible for merge workflow.

### 16.4 Review standard

Before merge:

- implementation-specific claims must match the repository;
- automated checks must pass;
- manual validation must cover correctness-sensitive UX;
- integrity failures must be resolved;
- documentation must distinguish current capability from future direction;
- no approval state is required in Moraine.

---

## 17. Immediate next action

**Milestone 0 (closed):** Vision realignment and decision de-centering — documentation, primary UI, and process language are ledger-only; historical decisions remain for compatibility.

**Next:** Complete **Local MCP Transport and Codex Integration** (Milestone 1 acceptance), then evidence capture.

Do not build additional approval semantics, live collaboration features, observability dashboards, or broad editor behavior.

The guiding sequence is:

```text
Vision and terminology alignment
        ↓
Zero-friction MCP agent integration
        ↓
Minimal trustworthy evidence capture
        ↓
Human findings and agent amendments
        ↓
Local run discovery and ledger-focused UX
        ↓
Second agent integration and external beta
```

---

## 18. Final product invariant

> Moraine preserves the durable record of agent work and the human context around it. It does not decide whether the work is accepted.

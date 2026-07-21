# Moraine Development Blueprint

**Status:** Canonical product and architecture direction for continued development  
**Repository:** `https://github.com/v-t-r-gg/Moraine`  
**Product:** Moraine  
**Purpose:** Define the path from the current agent-run protocol foundation to a low-friction, local-first ledger product that captures coding-agent work without requiring the desktop to remain open.

This document supersedes earlier blueprint versions where they conflict.

---

## 1. Current implementation status

At the time of this revision:

- **Milestone 0 — Vision realignment and decision de-centering** is merged on `main`.
- **Milestone 1 — Local MCP transport and Codex integration** is merged on `main`.
- **Milestone 2 — Local integration runtime and deterministic session capture** is **in progress** on `main`:
  - local service with Unix-socket intake, bounded spool, dedupe, index rebuild, and systemd `--user` helpers;
  - core provisional-run create/confirm with capture coverage and session envelopes;
  - Codex hook adapter (`moraine hook-codex`) for SessionStart / UserPromptSubmit / Stop.
- Remaining M2 work includes desktop notifications, fuller diagnostics/`doctor`, and dogfood hardening against live Codex hook payloads.

The architectural requirement remains an always-available **local integration runtime** that allows deterministic capture while the desktop is closed.

---

## 2. Executive summary

Moraine is a **local-first ledger for autonomous agent work**.

Its primary conceptual object is an **agent run**: a bounded unit of work performed by an agent. Moraine preserves what the agent did, why it did it, what evidence exists, what risks or unresolved questions remain, and what human context accumulated around the work.

Moraine is not an approval system. It does not decide whether work is accepted, rejected, mergeable, deployable, or authorized. Those decisions remain in the coding-agent session, pull request, issue tracker, CI system, or other workflow that already owns them.

The product invariant is:

> **Moraine preserves the record and review context; it does not render the verdict.**

The near-term product focus is coding agents:

> **Moraine is a local-first, cross-agent ledger for coding-agent work.**

Near 1.0, Moraine should be installed once and then remain mostly invisible during development:

1. The user configures a supported coding agent once.
2. The user continues working in that agent normally.
3. Moraine captures the session envelope and mechanical events automatically.
4. The agent contributes sparse semantic checkpoints through MCP.
5. Durable Markdown, sidecar metadata, and evidence remain beside the project.
6. The desktop is opened only when the human wants to inspect, comment, or add context.

The desktop must not need to remain open for capture.

---

## 3. Near-1.0 user experience

### 3.1 One product, several cooperating surfaces

A near-1.0 installation contains:

```text
Moraine product
├── `moraine` CLI
├── local integration service
├── local MCP server
├── agent hooks/adapters
├── desktop application
└── optional localhost web review surface
```

These are not separate editions. They are interfaces over one core product.

| Component | Primary audience | Purpose |
|---|---|---|
| CLI | users, scripts, CI | Setup, diagnostics, integration, inspection, export |
| MCP server | coding agents | Sparse semantic run operations |
| Hooks/adapters | supported agent hosts | Deterministic lifecycle and mechanical-event capture |
| Local service | background runtime | Event intake, reconciliation, indexing, notifications |
| Desktop | humans | Browse, inspect, comment, challenge, and add notes |
| Markdown + sidecar + evidence | users and tools | Durable portable run bundle |

### 3.2 Normal installation

The user installs one Moraine package. Platform packaging may create separate internal binaries, but the user should experience one product.

Representative setup:

```bash
moraine setup
cd /path/to/project
moraine project init
moraine integrate codex
```

The setup flow should:

- initialize project-local Moraine metadata;
- configure the local service;
- configure the selected agent integration;
- install or print the required MCP and hook configuration;
- verify project confinement;
- verify that events can reach the local service;
- avoid modifying unrelated project files without explicit consent.

### 3.3 Normal daily use

After one-time setup:

```bash
cd /path/to/project
codex
```

The user gives an ordinary coding task. No Moraine-specific task prompt is required.

During work:

- only the coding-agent window needs to be open;
- the agent host launches the configured STDIO MCP process when needed;
- deterministic hooks send lifecycle and tool events to the local service;
- the desktop may remain closed;
- the service persists or safely spools events;
- the project-local run bundle remains the durable source.

Afterward, the user may open the desktop to inspect the run.

### 3.4 Headless use

The desktop is optional.

A user should be able to use:

```bash
moraine runs
moraine show <run-id>
moraine export <run-id>
moraine doctor
```

A headless environment may use CLI output and project-local artifacts only.

### 3.5 Optional local web surface

A later local-only command may provide:

```bash
moraine web
```

This may serve the review interface on `localhost` for headless machines or browser-preferring users.

A hosted or remotely accessible web service is not required for the initial 1.0 scope.

---

## 4. Vision

### 4.1 Long-term vision

> Moraine is a local-first ledger for autonomous agent work.

Agents perform bounded runs. Moraine records their actions, implementation choices, outcomes, risks, unresolved questions, and evidence in durable files beside the work. Humans inspect those records, add comments and context, challenge claims, and preserve important review discussion.

The resulting artifacts remain readable and portable without requiring the original agent product, chat transcript, hosted service, or proprietary session viewer.

### 4.2 Near-term product position

> Moraine creates durable, source-adjacent records of coding-agent work that remain useful after the original agent session is gone.

Concise positioning:

> **Review agent work without relying on agent chat.**

### 4.3 Core value proposition

Moraine should help a developer answer:

- What task did the agent perform?
- What meaningful actions did it take?
- Why did it choose this approach?
- What changed?
- What evidence supports its claims?
- What commands and tools were actually observed?
- What failed or remains uncertain?
- What concerns or clarifications did humans raise?
- How did the record evolve after feedback or resumed work?

Moraine should not answer:

- Was this work formally approved?
- May this pull request merge?
- May this deployment proceed?
- Did an authenticated authority authorize the work?

---

## 5. Product principles

### 5.1 Ledger, not workflow gate

Moraine records the work and the review context around it. It does not become another approval gate layered on top of the agent session, GitHub, CI, or deployment tooling.

### 5.2 Agent run, not document

The conceptual object is the run. Markdown is a durable representation of the run, not the product’s fundamental unit.

### 5.3 Local-first and source-adjacent

Run artifacts live beside the work and remain controlled by the user. They may be committed to Git, left local, archived, copied, or inspected with ordinary tools.

### 5.4 Capture must not depend on the desktop

The desktop is a human review surface, not the capture runtime.

Closing the desktop must not stop a configured agent integration from recording run activity.

### 5.5 Tool independence

Moraine must not depend on one agent vendor. Codex, Claude Code, local agents, scripts, and future tools should use the same core run protocol and event model.

### 5.6 Human-readable durability

A run must remain understandable through its Markdown record even when Moraine is not installed. Structured state may support safe continuation and rendering, but the human-readable artifact must not become an empty shell.

### 5.7 Sparse semantic capture

Agents should record only review-relevant meaning. Moraine should mechanically capture facts that do not require model judgment.

### 5.8 Deterministic capture where supported

For supported and configured agents, session lifecycle and mechanical events should be captured through deterministic integration points rather than relying solely on model compliance.

### 5.9 Graceful degradation

Moraine must distinguish complete and partial capture rather than pretending every record has equal coverage.

### 5.10 Rebuildable runtime state

The local service may maintain indexes, queues, and notification state, but project-local run bundles remain canonical. Runtime state must be rebuildable from durable files wherever practical.

### 5.11 Honest trust boundaries

Agent-authored statements are claims. Agent-reported tests are not equivalent to Moraine-captured command results. Linked evidence is not automatically verified. Moraine is not tamper-proof, authenticated, signed, or compliance-grade unless those properties are explicitly implemented later.

### 5.12 Review without verdict

Human review may involve reading, commenting, challenging evidence, requesting clarification, responding to risks, and adding context. It does not need to end in an approval state.

---

## 6. Initial market niche

### 6.1 First users

The first target users are:

- individual developers who regularly use coding agents;
- open-source maintainers reviewing agent-generated changes;
- small engineering teams using more than one coding agent;
- local-model and privacy-conscious developers;
- developers running long, interrupted, or concurrent agent tasks;
- developers who want durable context independent of a vendor’s session history.

### 6.2 Market gap

Moraine occupies the gap between:

- ephemeral or vendor-specific agent chats;
- Git history and pull requests, which show changes but not the full work rationale;
- full observability systems, which capture traces but do not provide a concise source-adjacent human review record.

Moraine’s differentiation is the combination of:

- local ownership;
- cross-agent compatibility;
- automatic capture for supported integrations;
- durable plain files;
- source-adjacent records;
- sparse semantic checkpoints;
- evidence provenance;
- human comments and context;
- portability beyond the originating agent session.

### 6.3 What Moraine is not competing with

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

## 7. Canonical product model

### 7.1 Agent session

An **agent session** is a process or conversation lifecycle exposed by an agent host.

A session may contain:

- no bounded run;
- one run;
- several sequential runs;
- overlapping subagent activity.

A session is capture context. It is not automatically the durable product object.

### 7.2 Agent run

An **agent run** is a bounded unit of work performed by an agent.

Examples:

- implement one feature;
- investigate one defect;
- perform one migration attempt;
- review one subsystem;
- update one documentation area;
- diagnose one production problem.

A long interactive session may contain multiple runs.

### 7.3 Provisional run

A **provisional run** is created mechanically when a supported agent session begins substantive work before the agent has explicitly called `run_start`.

Its purpose is capture continuity.

A provisional run may derive an initial objective from:

- the initial user task;
- session metadata;
- the first substantive action.

When the agent later calls `run_start`, Moraine must reconcile with the provisional run rather than creating a duplicate.

If no semantic start call occurs, the provisional run remains a factual ledger with partial semantic coverage.

### 7.4 Run bundle

The complete durable object is the **run bundle**:

```text
Run bundle
├── Structured run state and operations
├── Human-readable Markdown projection
├── Human-controlled notes
├── Comments and review findings
├── Evidence references or captured evidence
├── Session and capture metadata
└── Sidecar identity, revisions, and compatibility metadata
```

### 7.5 Run record

The run record is the human-readable Markdown representation of the run bundle.

It should include:

- objective;
- lifecycle state;
- capture coverage;
- starting project and Git context;
- chronological checkpoints;
- actions;
- rationale;
- evidence;
- risks;
- unresolved questions;
- human notes.

### 7.6 Structured sidecar

The sidecar stores machine-managed state required for:

- stable run identity;
- session association;
- structured checkpoints;
- safe targeted mutation;
- idempotency;
- revision preconditions;
- recovery;
- lifecycle;
- capture coverage;
- annotations and findings;
- evidence metadata;
- schema compatibility.

### 7.7 Checkpoint

A checkpoint is a sparse, meaningful record of a development boundary.

A checkpoint may contain:

- concise summary;
- actions completed;
- implementation rationale;
- evidence references;
- risks;
- open questions;
- mechanically captured Git context.

A normal development run should generally require approximately three to eight semantic checkpoints, not a diary entry for every tool call.

### 7.8 Mechanical event

A mechanical event is observed through a hook, wrapper, service, or trusted adapter without requiring the frontier model to narrate it.

Examples:

- session start or end;
- tool invocation;
- command;
- working directory;
- exit status;
- file modification;
- Git transition;
- subagent start or stop;
- timestamp.

Mechanical events should not all become Markdown entries. Moraine should summarize or attach them as evidence while preserving enough structured detail for inspection.

### 7.9 Evidence

Evidence is information supporting or contextualizing a run claim.

Initial provenance categories:

- `agent_reported`: supplied by the agent without independent capture;
- `moraine_captured`: captured directly by a trusted Moraine operation or integration;
- `external_reference`: link or path to an external system or artifact;
- `human_context`: contextual evidence or note supplied by a human.

Moraine must present provenance clearly and must never silently upgrade agent-reported evidence to captured evidence.

### 7.10 Human notes

Human notes are free-form human-controlled Markdown preserved exactly. They are contextual ledger content, not an approval mechanism.

### 7.11 Review finding

A review finding is a durable human observation attached to a run, checkpoint, rationale, risk, question, or evidence item.

Possible finding kinds:

- clarification requested;
- evidence requested;
- possible error;
- risk noted;
- context;
- follow-up.

Findings are informative and conversational. They do not authorize or block work.

### 7.12 Lifecycle

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

### 7.13 Capture coverage

Capture coverage is separate from lifecycle.

Representative states:

- `full`: deterministic session envelope, mechanical events, and semantic checkpoints are present;
- `mechanical_only`: deterministic facts exist, but semantic checkpoints are missing;
- `semantic_only`: agent checkpoints exist, but deterministic hook capture is unavailable;
- `partial`: some configured capture channels failed or were unavailable;
- `unknown`: legacy or imported record without coverage metadata.

The UI and Markdown should show this honestly.

---

## 8. Human interaction model

| Area | Human action |
|---|---|
| Objective and checkpoints | Read and comment |
| Rationale | Comment or request clarification |
| Evidence | Inspect, challenge, or request more |
| Risks and questions | Comment and respond |
| Human notes | Directly edit |
| Run lifecycle | Observe |
| Capture coverage | Inspect |

### 8.1 Managed-region authority

For protocol-created runs:

- structured sidecar state is canonical for machine-managed regions;
- Markdown managed regions are deterministic projections;
- managed regions are read-only in the desktop application;
- comments remain allowed on managed content;
- direct text replacement and suggestion acceptance must not rewrite managed content;
- only the Human notes region is free-form editable.

Legacy free-form Markdown records may retain their existing editing behavior.

### 8.2 Agent amendments

When a human identifies an inaccurate or incomplete statement, the agent should amend the ledger through a protocol operation. Historical claims should not be silently rewritten without an identifiable operation.

A future amendment model should preserve:

- original checkpoint identity;
- human finding;
- agent response;
- amendment timestamp;
- superseded or clarified relationship.

---

## 9. Agent and capture interaction model

### 9.1 Semantic channel: MCP

The MCP transport captures information only the agent can meaningfully provide:

- objective refinement;
- rationale;
- significant actions;
- interpretation of failures;
- risks;
- unresolved questions;
- concise outcome.

The near-term tool surface remains:

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

### 9.2 Deterministic channel: hooks and adapters

Hooks and adapters capture facts without requiring model output:

- session envelope;
- initial task where available;
- tool and command events;
- timestamps;
- working directory;
- result status;
- file changes;
- final repository state;
- integration and agent identity labels.

Hook payloads must be validated, bounded, and treated as untrusted input from a local integration boundary.

### 9.3 Automatic provisional start

For supported integrations:

1. Session start registers a session envelope.
2. The first substantive event creates a provisional run when no run exists.
3. An MCP `run_start` call confirms or refines that run.
4. The same session and objective must not create duplicate runs.
5. If the agent never calls MCP, the mechanical run remains durable and is labeled accordingly.

### 9.4 Token-efficiency requirements

Normal operation should:

- avoid returning full Markdown after mutations;
- avoid resubmitting Git facts;
- avoid full-record reads;
- keep success responses compact;
- capture only meaningful semantic checkpoints;
- mechanically render readable Markdown;
- bound resume packets as runs grow;
- keep mechanical capture outside frontier-model context unless requested.

Initial product target:

> Moraine-related frontier-model overhead should remain below five percent for a normal coding task.

This is a target to measure, not a current marketing claim.

---

## 10. Local integration runtime

### 10.1 Purpose

The local integration runtime is the always-available capture layer for configured agents.

It allows:

- the desktop to remain closed;
- hooks to deliver deterministic events;
- events to be ordered and reconciled;
- project and run discovery;
- desktop notifications;
- recovery after temporary process failure.

### 10.2 Responsibilities

The service should:

- listen on a local Unix domain socket or Windows named pipe;
- authenticate access using local-user filesystem permissions and scoped tokens where appropriate;
- receive bounded hook and adapter events;
- resolve configured projects;
- create or reconcile provisional runs;
- invoke `moraine-core` operations;
- maintain a rebuildable project/run index;
- queue desktop notifications;
- expose health and diagnostic status;
- reconcile spooled events after restart.

### 10.3 Non-responsibilities

The service is not:

- the canonical ledger store;
- a cloud service;
- a network listener by default;
- an agent orchestrator;
- a shell execution service;
- an approval authority;
- a replacement for project-local files.

### 10.4 Canonicality

Canonical data remains in project-local run bundles.

The service may maintain:

- cached project locations;
- run summaries;
- unread or notification state;
- event-delivery cursors;
- integration health state.

If the service index is deleted, Moraine should rebuild it by scanning configured project metadata and sidecars.

### 10.5 Event spool

Hooks must not silently lose events when the service is unavailable.

A supported adapter should:

1. attempt delivery to the local service;
2. on failure, append a bounded event to a secure local spool;
3. return quickly enough not to disrupt the agent;
4. reconcile the spool when the service becomes available;
5. deduplicate using stable event identifiers.

The spool is recovery infrastructure, not the long-term ledger.

### 10.6 Service lifecycle

Near 1.0 should support platform-native user startup:

- `systemd --user` on supported Linux systems;
- LaunchAgent on macOS;
- appropriate per-user startup on Windows.

The CLI should provide:

```bash
moraine service install
moraine service start
moraine service stop
moraine service status
moraine service logs
moraine doctor
```

Exact commands may vary by platform, but service management must not require users to manage a terminal window.

---

## 11. Automatic capture contract

Moraine may claim automatic capture only for supported and correctly configured integrations.

### 11.1 Capture layers

#### Layer 1 — Session envelope

Always captured where the integration supports it:

- agent and integration;
- project;
- session identifier;
- start time;
- initial task where available;
- end time;
- final repository state.

#### Layer 2 — Mechanical event stream

Captured without model narration:

- commands;
- tool invocations;
- result status;
- working directory;
- file changes;
- Git transitions;
- evidence artifacts.

#### Layer 3 — Semantic checkpoints

Agent-supplied:

- rationale;
- significant decisions;
- interpretation;
- risk;
- uncertainty;
- concise outcome.

### 11.2 Graceful degradation

| Available channels | Result |
|---|---|
| Hooks + MCP | Full ledger |
| Hooks only | Factual ledger with partial interpretation |
| MCP only | Semantic ledger with weaker mechanical evidence |
| Wrapper only | Session envelope and wrapper-observable facts |
| No supported integration | No automatic capture guarantee |

The UI must display which channels contributed to the run.

### 11.3 Privacy boundary

Automatic capture must not imply full transcript capture.

By default, Moraine should not store:

- private model reasoning;
- full agent transcripts;
- arbitrary environment variables;
- secrets;
- complete command output without bounded policy;
- unrelated filesystem contents.

Capture policy must be configurable per project.

---

## 12. Architecture

### 12.1 Near-1.0 target shape

```text
Codex / Claude Code / supported agent
        │
        ├── MCP ───────────── sparse semantic operations
        │
        ├── lifecycle hooks ─ deterministic mechanical events
        │
        └── optional wrapper
                    │
                    ▼
            Moraine local service
                    │
        ┌───────────┼────────────┐
        │           │            │
   core run ops   evidence    rebuildable index
        │           │            │
        └───────────┼────────────┘
                    ▼
          project-local run bundle
        ├── run.md
        ├── run.md.moraine.json
        └── evidence/
                    │
        ┌───────────┴───────────┐
        ▼                       ▼
   Moraine desktop          Moraine CLI
```

### 12.2 Core rules

Business logic belongs in `moraine-core`.

The CLI, MCP server, local service, Tauri commands, and future adapters must call the same core operations. They must not maintain separate persistence or lifecycle rules.

### 12.3 Transport boundaries

- MCP remains local STDIO for the initial product.
- Hooks communicate with the local service through local IPC.
- Desktop communicates with the service for discovery and notifications, but may read run bundles directly through core operations.
- CLI can operate directly on project files when the service is absent.
- No transport becomes the canonical store.

### 12.4 Operation-based mutation

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

### 12.5 Append-oriented semantics

The sidecar should conceptually retain an append-oriented history of important run operations:

- session observed;
- provisional run created;
- run confirmed;
- checkpoint recorded;
- risk recorded;
- evidence linked;
- run marked ready;
- run resumed;
- finding added;
- finding answered;
- amendment recorded.

A materialized state may be retained for efficient reads and rendering. Event semantics should remain identifiable and replay-safe.

### 12.6 Plain-file durability

No central database is required for canonical project state.

A small local runtime database or index is acceptable only as rebuildable cache and delivery state. The product must remain recoverable from project-local run bundles.

### 12.7 Live collaboration

The existing Yjs/WebSocket collaboration capability is secondary.

Freeze broad live-collaboration investment unless real use demonstrates that it is necessary. Do not let relay, rooms, browser authority, or rich collaborative editing dominate the roadmap.

---

## 13. Existing run-decision capability

The repository contains historical run-level decisions such as `approved`, `changes_requested`, and `rejected`. These are no longer part of the core product vision.

### 13.1 Compatibility policy

Do not delete historical decision data abruptly.

Near-term policy:

- preserve existing decision fields during schema reads and migrations;
- preserve old records exactly;
- stop extending decision functionality;
- remove decision language from the product headline, vision, primary workflow, and examples;
- remove or hide decision controls from the primary desktop experience;
- retain `decide` only as legacy or compatibility functionality while needed;
- do not expose decisions through MCP, hooks, the local service, or future agent transports;
- do not require decisions in development-process gates;
- do not create new product features around decision freshness.

A later compatibility milestone may remove new decision creation while retaining read-only display of historical records.

### 13.2 Replacement concept

Replace decision-centric review with:

- comments;
- findings;
- evidence challenges;
- human notes;
- agent responses;
- amendments;
- descriptive lifecycle;
- capture coverage.

Moraine records what reviewers observed and how the record changed. External systems retain responsibility for final disposition.

---

## 14. Minimum viable product

A credible MVP should support this workflow:

```text
1. Install Moraine once
2. Initialize or connect a repository once
3. Configure a supported coding agent once
4. Give the agent an ordinary coding task
5. A session hook registers the session
6. Moraine creates or reconciles a run automatically
7. The agent records 3–8 sparse checkpoints through MCP
8. Mechanical Git and tool context is captured without model narration
9. Important command/test evidence is captured or linked
10. The desktop may remain closed throughout the run
11. Human later opens Moraine and finds the run by project
12. Human comments, challenges evidence, or adds notes
13. Agent can read findings and record responses or amendments
14. Durable Markdown + sidecar + evidence remain beside the project
```

### 14.1 MVP requirements

- stable agent-run protocol;
- local MCP transport;
- Codex integration;
- local integration service;
- deterministic hooks for at least one agent;
- automatic provisional-run creation and reconciliation;
- safe event spool and deduplication;
- zero Moraine-specific per-task prompt text;
- compact semantic checkpoints;
- basic captured command/test evidence;
- project and run discovery;
- simple active/ready/recent run list;
- durable findings and human notes;
- agent-readable findings and amendment flow;
- straightforward installation and service management;
- at least one second tested agent integration before public beta;
- complete demo repository and walkthrough.

### 14.2 MVP non-requirements

- formal approval states;
- remote MCP;
- hosted multi-user service;
- authenticated remote identities;
- cryptographic signatures;
- full trace ingestion;
- agent orchestration;
- advanced history UI;
- live-collaboration hardening;
- compliance features;
- enterprise policy enforcement.

---

## 15. Development roadmap

### Milestone 0 — Vision realignment and decision de-centering

**Status:** Complete on `main`.

**Goal:** Make the repository accurately express the ledger-only product boundary.

Key outcomes:

- review is inspection, comment, challenge, context, and response;
- decision controls are removed from the primary desktop path;
- legacy decision data remains compatible;
- no agent tool grants decision authority.

### Milestone 1 — Local MCP transport and Codex integration

**Status:** Complete on `main`.

**Goal:** Eliminate manual run setup and per-task prompt ceremony for semantic capture.

Key outcomes:

- local STDIO MCP server over core run operations;
- five tools only: start, show, checkpoint, ready, resume;
- fixed project confinement;
- concise server instructions;
- Codex integration documentation;
- no full Markdown in normal responses;
- no decision tool;
- MCP included in CI and setup guidance.

### Milestone 2 — Local integration runtime and deterministic session capture

Milestone 2 — Local integration runtime and deterministic session capture

Status: Complete

Validated:
- desktop-closed capture;
- provisional-to-semantic reconciliation;
- multiple runs per session;
- durable replay deduplication;
- service-down spooling;
- project confinement;
- descriptive session-stop semantics;
- derived capture coverage;
- index reconstruction.

### Milestone 3 — Minimal trustworthy evidence capture

**Goal:** Move important verification facts from agent claim to mechanically captured evidence.

Scope:

- exact command where exposed by the integration;
- working directory;
- timestamps;
- exit code or result status;
- selected bounded output artifact;
- output hash;
- current Git head and changed-file summary;
- external URL/path evidence references;
- clear provenance rendering;
- configurable capture and redaction policy.

Non-goals:

- full terminal recording;
- full observability traces;
- prompt or model telemetry;
- arbitrary remote execution.

Acceptance criteria:

- a reviewer can distinguish agent-reported from captured evidence;
- captured evidence is linked into the run without large model payloads;
- failure output is preserved honestly;
- secrets and unrelated environment data are not captured by default;
- no command is claimed as captured unless Moraine directly observed it.

### Milestone 4 — Findings and amendment loop

**Goal:** Let human review context flow durably between the desktop and agent without introducing verdicts.

Status: **M4 checkpoint findings implemented** (checkpoint-only targets, MCP list/get/respond, desktop thread). Amendment operations and non-checkpoint targets remain later slices. Findings-capable sidecars use schema **v5**.

Scope:

- typed review findings;
- findings attached to checkpoints, rationale, evidence, risks, or questions;
- open/addressed/archived descriptive state;
- MCP tools for listing and responding to findings;
- agent amendment operations;
- durable relationship between original claim, finding, response, and amendment.

Acceptance criteria:

- a human can challenge a claim in the desktop;
- the agent can read the finding through its transport;
- the agent can respond or amend the run;
- the ledger preserves the complete exchange;
- no approval or rejection state is introduced.

### Milestone 4.5 — React desktop migration

**Goal:** Replace the Svelte/SvelteKit desktop frontend with React + TypeScript + Vite without changing the run model or product features.

Status: **implemented** (React is the desktop framework).

### Milestone 4.6 — Append-only ledger semantics

**Goal:** Protocol runs are append-only for human/agent review context; checkpoints and agent claims are immutable; free-form Human notes are not the durable write path.

Status: **complete**. Operations: `human_observation_add`, `run_amend`, `entry_supersede`, `entry_redact`. Desktop: structured read-only timeline + Add observation; Legacy document mode for free-form Markdown only. Sequential amendments freeze the immediately prior claim.

### Milestone 5 — Local run discovery and ledger-focused desktop UX

**Goal:** Make the desktop useful across multiple projects and runs without becoming the capture dependency.

Status: **current** (implemented on `feat/local-run-discovery-ledger-ux`).

Scope:

- project list from the rebuildable service index (noncanonical cache);
- active, ready, and recent run lists via core read models;
- filters for lifecycle category, capture coverage, findings, risks, and unresolved questions;
- structured ledger timeline (checkpoints, evidence, findings, observations, amendments, supersessions, redactions);
- evidence and findings inspection through existing panels;
- append-only human observations / amend / supersede / redact (no free-form Human notes editor for protocol runs);
- Legacy document mode retained for free-form Markdown only;
- index rebuild and project rescan controls (index-only mutation);
- offline/direct filesystem inspection when the local service is unavailable.

Acceptance criteria:

- a user does not need to know a run path;
- runs remain discoverable after restart;
- the desktop can be closed without affecting capture;
- project scanning does not mutate run records;
- the UI emphasizes ledger inspection rather than document authoring;
- the service index is rebuildable and never treated as canonical run storage.

### Milestone 6 — Second agent integration, packaging, and external beta

**Goal:** Prove vendor-neutral value and make installation reproducible.

Scope:

- second tested agent, preferably Claude Code or another hook- and MCP-capable tool;
- platform packaging for CLI, service, MCP, and desktop;
- service install/uninstall flow;
- versioned configuration and migrations;
- polished demo repository;
- screenshots and short video walkthrough;
- five external developer testers;
- structured feedback and repeat-use measurement.

Acceptance criteria:

- at least two agents can create equivalent run bundles;
- setup is achievable without maintainer assistance;
- capture works without keeping Moraine open;
- external users voluntarily use Moraine for another task;
- most normal runs are understandable without reopening the full agent transcript.

---

## 16. Quality and trust requirements

### 16.1 Data integrity

Immediate blockers:

- data loss;
- ghost or duplicated checkpoints;
- duplicate provisional and semantic runs;
- wrong restored content;
- silent last-writer-wins loss;
- evidence provenance escalation;
- human notes overwritten;
- annotations or findings silently disappearing;
- unrecoverable incomplete operations;
- silently dropped hook events;
- event replay producing duplicated ledger facts.

### 16.2 Compatibility

- preserve existing v3/v4 sidecars;
- reject unsupported future schema versions;
- migrate deterministically;
- never delete historical decisions or annotations during ordinary migration;
- test LF/CRLF and exact-byte Human notes preservation;
- maintain read-only compatibility for legacy records;
- version hook event and service IPC schemas.

### 16.3 Security

Near-term security model:

- local trusted-user environment;
- no public network listener;
- project-confined filesystem access;
- local IPC protected by user permissions;
- no arbitrary path writes;
- no hidden shell execution through ledger tools;
- no secret or full-transcript ingestion by default;
- bounded event payloads;
- explicit capture and redaction policy;
- no claim of authenticated agent identity.

### 16.4 Performance and token cost

Measure:

- bytes returned by MCP tools;
- estimated model token overhead;
- checkpoint count;
- full-record reads;
- hook event volume;
- service queue depth;
- event reconciliation latency;
- Markdown rendering time;
- project scan time;
- evidence artifact size.

Normal tool results should remain compact and growth-bounded. Hook capture should not inject mechanical events into frontier-model context unless requested.

### 16.5 Availability

The local service should fail safely:

- agent work must continue if Moraine is temporarily unavailable;
- configured hooks should spool bounded events;
- service restart should reconcile without duplication;
- corrupted spool entries should be isolated and reported;
- the CLI should provide a useful diagnostic path.

---

## 17. Product success metrics

Initial targets:

| Metric | Target |
|---|---:|
| Initial project setup | Under 5 minutes |
| Moraine-specific text in normal task prompt | Zero |
| Additional capture windows during work | Zero |
| Desktop required during capture | No |
| Frontier-model token overhead | Below 5% |
| Standard semantic checkpoints per run | 3–8 |
| Full Markdown reads by agent | Normally zero |
| Supported-session envelope capture | At least 99% in dogfood |
| Hook events silently lost | Zero |
| Duplicate provisional/confirmed runs | Zero |
| Runs understandable without transcript | At least 80% of normal tasks |
| Small-run human inspection time | Under 5 minutes |
| Data-integrity failures | Zero |
| Tested coding-agent integrations for beta | At least 2 |
| Repeat use by external testers | Majority voluntarily use again |

These are product targets and must not be presented as achieved until measured.

---

## 18. Portfolio showcase definition

A flagship portfolio release should demonstrate the complete product loop:

- stable protocol and persistence architecture;
- working MCP integration with a real coding agent;
- deterministic hook capture;
- background service with no second terminal window;
- one normal prompt producing a run automatically;
- provisional-run and semantic-run reconciliation;
- captured test evidence;
- desktop project/run discovery;
- human comments/findings and agent amendments;
- durable Markdown and sidecar visible in the demo repository;
- clean installation and service-management instructions;
- architecture diagram;
- concise trust and limitation statement;
- screenshots and a short demonstration video;
- a real case study comparing the run record with the original agent transcript.

The current project is a strong protocol showcase after M0/M1, but not yet a polished end-user product. The largest missing architectural bridge is the local integration runtime and deterministic capture path.

---

## 19. Explicit non-goals

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
- a generalized enterprise governance platform;
- a default full-transcript recorder.

---

## 20. Development process

### 20.1 Bounded milestones

Each milestone should solve one observed product problem and avoid speculative adjacent work.

### 20.2 Dogfooding

Dogfood the interface being built, not obsolete manual scaffolding.

Examples:

- MCP work should be tested through MCP;
- service work should be tested with the desktop closed;
- hook work should be tested through real agent lifecycle events;
- evidence capture should be tested on real commands;
- findings should be tested through a human-to-agent exchange;
- run discovery should be tested with a real multi-run project.

### 20.3 Run records

Moraine development should continue using its own run protocol, but no human decision is required as a product or process gate.

A development run should retain:

- objective;
- scope;
- starting state;
- meaningful semantic checkpoints;
- captured mechanical evidence;
- risks;
- unresolved questions;
- human findings and notes;
- final descriptive lifecycle state;
- capture coverage.

GitHub pull requests and CI remain responsible for merge workflow.

### 20.4 Review standard

Before merge:

- implementation-specific claims must match the repository;
- automated checks must pass;
- manual validation must cover correctness-sensitive UX and runtime behavior;
- integrity failures must be resolved;
- documentation must distinguish current capability from future direction;
- no approval state is required in Moraine.

---

## 21. Immediate next action

1. Continue **Milestone 2** dogfood: run `moraine-service` with the desktop closed, configure Codex hooks, confirm provisional → MCP `run_start` reconciliation on a real task.
2. Add desktop notification queue and richer `doctor`/diagnostics once the capture path is stable in daily use.
3. Do not jump directly to a standalone `moraine exec` milestone before the capture runtime is dogfooded; evidence capture (M3) should build on the runtime and hook event model.
4. Keep live collaboration, hosted web, approval semantics, and broad observability out of scope.

The guiding sequence is:

```text
Vision and terminology alignment
        ↓
Zero-friction MCP semantic integration
        ↓
Always-available local runtime + deterministic hooks
        ↓
Minimal trustworthy evidence capture
        ↓
Human findings and agent amendments
        ↓
Ledger-focused desktop discovery and UX
        ↓
Second agent integration, packaging, and external beta
```

---

## 22. Final product invariant

> Moraine preserves the durable record of agent work and the human context around it. For supported integrations, it captures that record without requiring the desktop to remain open. It does not decide whether the work is accepted.

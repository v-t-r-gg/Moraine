# Vision

Moraine is a local-first ledger for coding-agent work.

## The problem

Coding agents can produce useful work without leaving a durable account of
their goals, actions, evidence, uncertainty or corrections. Chat transcripts
are hard to review, easy to lose & detached from the source they changed.

Moraine makes an agent run the conceptual object. A run is not a chat room or
an approval request; it is a durable record of work.

## Stable principles

### Local first

Canonical records live beside the project as ordinary files. They remain
readable without a hosted account, relay or proprietary database.

### Source adjacent

Run Markdown, structured sidecars & optional evidence stay close to the code
they describe. Teams choose whether to commit or share them.

### Desktop independent

Capture continues through the local service while the desktop is closed. The
desktop is a reader, navigator & repair surface; it is not the authority for
the ledger.

### Honest capture

Moraine distinguishes mechanical hook events, semantic agent checkpoints,
evidence references & missing coverage. It must not imply that an event was
captured when only an indirect signal exists.

### Append-only correction

Observations, amendments, supersessions & redactions preserve history. Current
views may hide or replace a claim; the ledger still records the operation that
changed its ordinary projection.

### Human review without verdict

People may inspect, comment, add findings & record observations. Moraine does
not authorize a merge, deployment, approval or rejection. External systems own
those decisions.

### Bounded integrations

Agent adapters map an external agent into one durable run protocol. The domain
model must not depend on one agent, operating system or desktop framework.

## Product boundaries

Moraine is:

* a durable ledger for agent runs;
* a local capture & discovery system;
* a review surface for evidence, findings & corrections;
* a transport-neutral run protocol with local integrations.

Moraine is not:

* an agent orchestrator;
* a merge gate or approval engine;
* a Git or pull-request replacement;
* a hosted compliance archive;
* a secret manager;
* a general knowledge workspace;
* a trusted multi-user network service.

The product succeeds when a person can understand an agent run from durable
project files, see what was & was not captured, add review activity without
rewriting history & keep working without a remote dependency.

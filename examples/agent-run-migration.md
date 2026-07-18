# Agent run: PostgreSQL migration dry-run (example)

**Run id:** example-2026-07-18-pg-migration  
**Status:** needs human review  
**Started / ended:** illustrative only  

## Objective

Assess whether the pending schema migration can be applied to staging without locking the `orders` table for more than a few seconds.

## Context

* Repo: example service monorepo  
* Target: staging Postgres 15  
* Agent had read-only credentials and migration files under `db/migrations/`.

## Actions taken

1. Listed pending migrations and read `db/migrations/20260718_add_order_index.sql`.
2. Estimated table size via `pg_stat_user_tables` (illustrative numbers below).
3. Ran `EXPLAIN` on the index creation statement in a transaction that was rolled back.
4. Did **not** apply the migration (no write grant on staging for DDL in this run).

## Decisions and rationale

* Recommend **CONCURRENTLY** for the index if Postgres version and migration tooling allow it; otherwise schedule a maintenance window.
* Do not run the non-concurrent form during peak traffic.

## Outcome

* Migration is low risk if concurrent index build is used.
* Blocking form would lock writes on `orders` for an unknown duration on large tables.

## Evidence / verification

| Item | Location / note |
|------|-----------------|
| Migration SQL | `db/migrations/20260718_add_order_index.sql` (in your tree) |
| EXPLAIN output | Attach or paste in a real run; omitted in this example |
| Row estimate | Example: `orders` ~ 12M rows (replace with real `status` query output) |

Moraine does not auto-collect these artifacts. The agent (or human) must write paths and results into the record.

## Risks

* Concurrent index build still consumes I/O.
* If tooling rewrites SQL and drops `CONCURRENTLY`, staging could stall.

## Unresolved questions

* Does the migration runner support `CREATE INDEX CONCURRENTLY` outside a transaction?
* Is there a replica lag SLO that would block a long concurrent build?

## Needs human review

* [ ] Use Moraine **Run review**: Approve / Request changes / Reject (bound to this Markdown revision)
* [ ] Approve concurrent approach or schedule a window for blocking DDL  
* [ ] Confirm staging is the correct target  
* [ ] Optional: Suggest clearer wording in the Outcome section via Moraine **Suggest**

Run-level decisions are separate from accepting a text suggestion. After Markdown changes, prior decisions become stale until re-recorded.

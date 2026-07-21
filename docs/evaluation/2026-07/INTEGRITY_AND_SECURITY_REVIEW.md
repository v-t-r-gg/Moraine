# Integrity and security review

**Baseline:** `4f8d1e8`  
See also [RISK_REGISTER.md](./RISK_REGISTER.md), [ARCHITECTURE_AND_INTEGRITY.md](./ARCHITECTURE_AND_INTEGRITY.md).

## Trust model (actual)

- Local single-user machine
- No network multi-tenant auth
- Filesystem owner can always edit plain files
- Redaction = ordinary-reader policy, not cryptographic erase
- No cryptographic agent identity

## Strengths (evidence)

| Control | Evidence |
|---------|----------|
| Per-record sidecar lock | `atomic` tests concurrent writes |
| Incomplete-op recovery | Findings survival tests |
| Idempotency keys | Protocol + finding respond tests |
| Spool seen markers | `spool.rs` durable_seen tests |
| Discovery nonmutation | `discovery::summarize_and_list_without_mutation` |
| Index rebuild nonmutation | `discovery_index` / live rebuild |
| Unsupported schema reject | `unsupported_schema_represented` |
| Loopback-only HTTP bind | Service refuses non-loopback |
| No decision tools in MCP | MCP tool set tests |

## Critical gap on main

**Finding DTOs expose frozen checkpoint summary/snapshot without redaction check.**

- Affects: `list_findings`, `get_finding`, `respond_to_finding`, desktop path APIs, MCP JSON
- Ordinary timeline/ProtocolLedgerPanel partially fixed on main
- Fix exists as open PR #12 (not baseline)
- **Classification:** integrity + security (sensitive content disclosure)
- **Beta blocker:** yes

## Other integrity notes

| Topic | Status |
|-------|--------|
| Index canonicality | Correctly noncanonical in design |
| External MD edit | Expected; do not market immutability |
| Provenance escalation | Evidence model tries to prevent; monitor |
| Cross-project writes | Confinement intended; not fully re-audited here |
| Secret exposure | Evidence scrubbing helpers; redaction gap is separate |
| Destructive future schema | Rejected — good |
| Silent LWW | Protocol uses locks/idempotency; incomplete-op path careful |

## Security documentation gap

No `SECURITY.md` / threat model document in repo root.  
Beta should document local-trust boundaries explicitly.

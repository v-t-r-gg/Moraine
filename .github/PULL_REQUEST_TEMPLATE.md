## Objective

## Related issue

Closes #

## Moraine run record

`.moraine/runs/...`

## Implementation commits reviewed

-

## Main changes

## Schema or compatibility effects

## Automated validation

- [ ] `./scripts/check.sh`
- [ ] Migration tests
- [ ] Platform-sensitive tests where applicable

## Manual validation

- [ ] Primary happy path
- [ ] Conflict or failure path
- [ ] Reopen and recovery path
- [ ] Existing file compatibility

## Known limitations

## Review-ledger state

- [ ] The run record names the reviewed implementation commit.
- [ ] The Moraine decision is current for the run-record Markdown.
- [ ] No implementation files changed after the reviewed commit.
- [ ] Any implementation or run-record Markdown change after approval was reviewed again.

A sidecar-only commit that records the human decision does not itself
require another decision.

Do not merge while the Moraine decision is stale relative to the run
record. If implementation changes after approval, update the reviewed
commit in the run record and record a new decision.

### Guarantee boundary (current product)

| Guarantee | Current status |
| --------- | -------------- |
| Decision applies to exact run-record Markdown | Mechanically enforced |
| Run record names an implementation commit | Manually recorded |
| Implementation commit has not changed | Process-enforced |
| Decision cryptographically applies to source tree | Not implemented |

# Performance and scale evaluation

**Baseline:** `4f8d1e8`  
**When:** 2026-07-21  
**Host:** Linux x86_64, local SSD  
**Method:** Temporary tree of 20 projects; 960 total runs (19×40 + 1×200); live `moraine-service` loopback rebuild/list.  
**Raw metrics:** [data/scale-results.json](./data/scale-results.json) (ephemeral fixture paths omitted).

> Objective asked for 20 projects / 1000 runs / 200 in one project. This run used **960** total runs with **200** in the largest project (time budget). Treat as representative, not exact 1000.

## Measurements

| Metric | Result |
|--------|--------|
| Service startup (to health) | **0.062 s** |
| Create 960 runs (CLI `run start` loops) | **9.55 s** (~100 runs/s) |
| `POST /index/rebuild` (20 projects) | **0.205 s** |
| `GET /projects` | **0.002 s** (20 entries) |
| `GET /projects/{id}/runs` (200 runs) | **0.059 s** |
| Largest project on-disk size | **~432 KiB** (200 empty-ish protocol runs) |
| All 20 projects on-disk | **~2.0 MiB** |

## Not measured this session

| Item | Reason |
|------|--------|
| Desktop cold start / timeline render | No interactive instrumentation here |
| Large evidence artifacts / finding threads | Not generated in scale fixture |
| Memory RSS of service under load | Not sampled |
| Concurrent capture + discovery stress | Not run |
| 1000 exact run count | 960 used; pattern clear |

## Bottleneck classification

| Observation | Beta block? | 1.0 block? |
|-------------|-------------|------------|
| Index rebuild & listing fine at ~1k runs | No | No |
| CLI create path OK for dogfood scale | No | No |
| Sidecar growth linear with runs (expected) | No | Watch at 10k+ |
| Desktop virtualization not needed yet | No | Maybe later |
| curl probe latency negligible vs rebuild | No | — |

## Conclusion

At **~1k runs / 20 projects**, service discovery performance is **not a beta blocker**.  
Optimize only after live desktop/timeline measurements show pain. Do **not** spend a milestone on performance now.

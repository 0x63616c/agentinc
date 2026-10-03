# CI and release timing audit — 2026-10-02

Measured from `gh run view RUN --json jobs` and `gh run view RUN --log`, not
local build timings. Both successful runs used release commit
`1d9a83e45a61baf1c356294297cc73f7de66694d` (0.3.5).

| Baseline | Measured elapsed | Finding / change |
| --- | ---: | --- |
| [CI 37062404366](https://github.com/0x63616c/agentinc/actions/runs/37062404366) | **23m18s** | One serial job; compilation dominates. |
| Generator | 9m30s Cargo build (9m31s step) | Checked by `generated_api_and_clients_are_current` inside the workspace test instead; no separate generator dependency graph/build. |
| Clippy | 5m14s Cargo (5m15s step) | Separate concurrent lane, still workspace/all-targets and warnings denied. |
| Workspace tests | 6m51s compile; ~45s execute | Complete workspace tests and doctests retained, including real Postgres/Temporal tests. |
| CI cache | 1,537,216,317 bytes; ~24s step | Exact immutable hit, then `Cache up-to-date`; Temporal and many other dependencies still recompiled. Rolling successful-main caches now retain each lane's complete target, not just the original dependency snapshot. |
| [Distribution 37062404185](https://github.com/0x63616c/agentinc/actions/runs/37062404185) | **39m17s** | Serial native → distribution → upgrade → publication. |
| Native job | 12m38s | Production handoff 4m06s; test fixtures 8m09s. Production Cargo release build 2m53s; test-key candidate 2m31s, newer 1m34s, prior 0.3.3 1m51s. Every eligible historical rebuild remains. |
| Native resources | Peak sampled 1,578 MiB RSS, 399% CPU | Four build workers were busy. No speculative build-worker increase. |
| Distribution job | 22m03s | Included ~10m15s **idle CI wait before signing**; signing now overlaps CI, with the gate before asset upload and again before publication. |
| Four signed/notarized bundles | ~8m00s serial (21:07:01–21:15:01) | Up to four independent bundle workers. Each still signs nested code, waits for acceptance, staples, and archives; any failure blocks progression. |
| Apple processing | 39s, 42s, 42s, 36s poll durations | Not the dominant baseline bottleneck. No notarization or Gatekeeper checks removed. |
| Linux manifest tool | 3m12s Cargo; ~1m of git/index updates | Build concurrently with signing, use the compile-efficient CI profile, and retain its own rolling cache. No change to Ed25519 key validation or manifest contents. |
| Upgrade job | 4m10s | ~33s downloading **all unsigned bundles** it never reads. Download only its key/manifest-tool artifact instead. Signed fixture download (~35s) and full upgrade gate remain. |
| Native upgrade exercises | ~2m49s gate step | Manual + automatic 0.3.3 → 0.3.5 and 0.3.5 → 0.3.6, with new/restored terminal checks, plus signature/Gatekeeper checks of every published predecessor. All retained. |

## Expected changes, not measured results

The new `ci` Cargo profile keeps debug assertions and overflow checking but sets
dependency optimization to zero. Local interactive dev optimization and shipping
release optimization are unchanged. Clippy and tests compile on independent
runners; `rust` remains the required aggregate check and fails on any failed,
cancelled, or skipped lane. Superseded CI runs cancel; Distribution never does.

The release critical path becomes approximately
`max(CI, native + signing/manifest staging) + upgrade + publication`, rather than
waiting for CI and then doing all signing/compilation serially. If the observed
CI/native times held and parallel staging fit inside the existing CI wait, the
baseline budget would be about **27–28 minutes instead of 39m17s**. This is a
scheduling estimate, **not an achieved improvement**. New profile cold caches,
concurrent compression/CPU contention, additional historical releases, runner
queues and Apple's service can change it. Do not add nominal savings together.

Only temporary unsigned handoffs use gzip level 1; public signed archives retain
their existing compression. This trades larger short-lived artifacts for less
native compression work; net transfer/build time must be measured on the runner.
Rolling caches use toolchain/manifest/commit keys and previous compatible lane
restore prefixes; only successful main jobs save (Distribution dispatches cannot
write the main cache). Cache eviction remains possible.
Historical fixtures intentionally are **not** cached under an unbound/random test
key; coverage is more important than claiming an unsafe cache hit.

## Validation and next measurement

- Python regression tests cover latest exact-commit CI selection (including failed
  reruns/cancellations), four-bundle/build overlap using an explicit barrier,
  failure propagation, fixture identity/inventory checks, required notary
  wait/staple, and CI failure preventing asset upload/publication.
- `actionlint` (custom runner label configured), Python compilation, Cargo metadata,
  Rust formatting and `git diff --check` pass locally. The CI gate also accepted
  the actual successful baseline run through GitHub's workflow-runs API.
- No new full Rust build or real signing/release run was performed by this worker;
  the coordinator owns consolidated workspace validation and the next authorized
  minor release. Signing now logs per-archive elapsed time. Compare its Actions
  job/step timestamps and cache hit/save logs against this baseline before claiming
  an achieved speedup.

Do not push a newer main commit during a release if its exact-commit CI is still
running: supersession cancels it and the release correctly refuses to publish.
The pre-existing `build=false` fallback only uploads a production archive, not the
upgrade fixtures/tools required by the native gate; it remains fail-closed and is
not an escape hatch around upgrade coverage.

## Measured CI follow-up — 2026-10-03

[CI 37082617976](https://github.com/0x63616c/agentinc/actions/runs/37082617976)
completed successfully for `27a7a2e81fdf342a556c01a13214c5994500d399`
(the 0.4.0 release commit). GitHub timestamps and completed logs show:

| Measurement | Observed result |
| --- | --- |
| Whole workflow | **8m15s** (00:34:11–00:42:26 UTC), versus 23m18s: **15m03s / 64.6% less elapsed time**. Includes ~51–53s initial runner queueing. |
| Concurrent Clippy lane | Job 4m27s; command step 3m49s; Cargo reported **3m40s**. |
| Workspace test lane | Job 7m15s; command step 6m31s; Cargo reported **5m19s** build, followed by **~63s** test/doctest execution (00:40:59–00:42:02). |
| Generated API/client check | Passed inside the xtask test suite, which finished in **0.31s**; no standalone generator build. |
| Cache restoration | **Both lanes missed**, with subsecond lookup steps. This result does not demonstrate warm-cache savings. |
| Successful cache saves | Clippy **9s**, 860,128,276 bytes (~820 MiB); tests **15s**, 1,347,029,572 bytes (~1,285 MiB). Restore effectiveness remains to be measured. |
| Safety checks | `checks` and aggregate `rust` succeeded; 15 Python tests passed. Rust result lines total 159 passed, zero failed, four ignored. |

Cargo durations include dependency fetching/index updates but exclude the ~8s
automatic pinned-toolchain installation in each command step. Test execution rose
from the baseline's ~45s to ~63s; this run also contains additional regression
tests and uses unoptimized dependencies. This is an observed comparison between
two release commits, not an isolated attribution of savings to each change.
No completed Distribution timing or publication result is claimed here.

# CI cache audit: 2026-10-17

Implementation and measurement tracker:
[obs-studio#1286](https://github.com/obs-rust/obs-studio/issues/1286).

## Executive assessment

The janitor failed because a fork PR received a read-only Actions token:
all 32 deletion requests returned HTTP 403. Maintenance also delayed useful
work through its dependency graph. The merged fix keeps `ci-pre.yml` as the
common pre-work entry point, runs only its janitor on trusted queue/master
events, and keeps maintenance outside build, required-gate and cancellation
dependencies. A maintenance failure remains visible without blocking a merge.

Compiler caching is effective. The remaining speed work is reducing needless
transfers, matching dependency snapshots to consumers, and shortening the test
critical path. Preserve useful caches and required integrity coverage; judge
changes by total gate time and runner cost rather than archive HIT messages.

### Verified rollout results

Master [39061960882](https://github.com/obs-rust/obs-studio/actions/runs/39061960882)
completed successfully with the following results:

| Area | Observation | Decision |
| --- | --- | --- |
| Linux Tier 2 | 774/775 compilations cached; build step 53 s; complete build job 162 s | Keep the working compiler cache |
| Qt frontend | 295/297 cacheable hits; restore 10 s, build/tests 55 s, save 6 s | Keep the 1.2 GB cache bound and realistic retention allowance |
| OFF/ON tests | 286 registered per mode: 283 passed, 3 skipped, 0 failed; concurrent phase 327 s wall / 642 runner-seconds | Keep both implementations and parallelize independent modes |
| Scheduling | Fast Gate 234 s; 3 s to build start; build 162 s; 3 s to test runners | Continue measuring runner waits as well as commands |
| Rust compiler correctness | Pinned soldr 0.9.35 still returns a stale success in the nested-source probe | Retain the wrapper bypass pending the upstream fix |

The earlier serial test job took 578 s for 283 tests per implementation; the
first parallel trial spanned 366 s and consumed 715 runner-seconds. The newer
327-second phase above registered 286 tests per mode (283 passed, three
PulseAudio device tests skipped). These are **unmatched phase
observations**, not proof of overall queue throughput improvement. Source,
runner placement and compiler-cache rollout conditions differ.

[Machine-readable rollout results](ci-evidence/2026-10-17-master-cache-rollout.json)
include job IDs, phase times and independent compiler counters. The isolated
libobs results below remain separate from whole-app and CI build timings.

### Native Rust profile verification

Master [39063249804](https://github.com/obs-rust/obs-studio/actions/runs/39063249804)
passed native Rust test jobs on both platforms with `--profile dev`:

| Platform | Setup | Cook inside setup | Rust tests | Post-job setup-soldr save |
| --- | --- | --- | --- | --- |
| macOS arm64 | 34 s | 23.1 s | 89 s | 13 s |
| Windows x64 | 53 s | 30.0 s | 125 s | 15 s |

[Raw job observations](ci-evidence/2026-10-17-native-rust-cook-rollout.json)
verify profile use and successful consumers. Compiler-wrapper session counters
were zero in both jobs; these are separate from dependency-cook reuse. The
parent master run was later canceled after its native jobs and cache writers
finished; this is not a full-workflow pass or a matched speed comparison.

Windows native Tier 2 also passed in this master run, using `--release` for
the static-library consumer and recording **304/305 cacheable ccache hits**,
one miss. It saved `windows-tier2-ccache-6d7b7f7a...`. These C/C++ counters
are independent of the zero embedded zccache session counters printed by
setup-soldr. [Job timings and counters](ci-evidence/2026-10-17-windows-tier2-cache-rollout.json)
confirm that useful native compiler reuse survived the changes.

macOS native Tier 2 passed both implementation runs too: 161 registered,
160 passed and one `test_svt_av1` skip each, with CTest times 96.45/101.15 s.
Its release-profile consumer succeeded and saved `macos-tier2-xcas-6d7b7f7a...`.
The embedded compiler session recorded 22 hits and zero misses; this does not
establish an Xcode compilation-cache hit rate.
[Native macOS observations](ci-evidence/2026-10-17-macos-tier2-cache-rollout.json)
keep these independent scopes explicit. The obsolete master workflow was
canceled after its native jobs and cache writers completed; it is not claimed
as a full-workflow success. Full nightly verification remains separate.

### What still dominates test time

The [serial-test profile](ci-evidence/2026-10-17-tier2-serial-profile.json)
identifies eight `RUN_SERIAL` tests totaling 174.47 s OFF and 165.17 s ON,
against CTest wall times of 257.61 s and 248.03 s. The longest are async audio
delay (~59 s), PipeWire resume (~39 s), VLC pacing (~35 s), and audio clock
drift (23.52 s OFF / 14.55 s ON). The independent mode runners are balanced
within 12 s of job wall time; dividing tests evenly by count would not balance
these durations. No test sharding or removal of serial isolation was shipped.
A future experiment can isolate long suites in separate environments, measuring
extra runner/setup cost and timing stability before changing their scheduling.

Both modes skipped `test_pulse_monitor_device`, `test_pulse_stream_restart`
and `test_pulse_two_sources`; a green workflow is not evidence those device
scenarios executed. Their environmental coverage remains a stated limitation.

### Queue admission and operating strategy

1. Queue admission covers compilation and integrity: selected builds, lint,
   tests, ABI/behavior and differential checks. Security scans and release
   packaging validation run nightly or through explicit manual/release runs.
   Automatic GitHub CodeQL setup is disabled; merged #1327 puts the same four
   languages in the existing Scheduled workflow and reports nightly failures.
2. Queue/master janitors own retention. PRs restore useful snapshots; remote
   setup-soldr saves are master-only. Local development saves remain enabled.
3. Protect correctness first: retain the Rust wrapper bypass and cross-lane
   toolchain safeguards. Do not trade stale-output protection for headline hits.
4. Preserve one active master warming build and the latest pending snapshot.
   Catch-up scope must retain paths from superseded pushes; #1328 implements
   that after the plugin seed exposed a lost-path case. Follow-up #1330
   protects the rollout baseline from older coalesced successes that did not
   cover replaced pending pushes.
5. Keep independent OFF/ON test runners and publish the extra runner cost.
   Keep all applicable test coverage; no test sharding was introduced.

Nightly security migration is live-verified: manual Scheduled run
[39064183327](https://github.com/obs-rust/obs-studio/actions/runs/39064183327)
passed CodeQL for Actions, C/C++, Python and Rust. Automatic default setup
remains disabled. [Per-language results](ci-evidence/2026-10-17-nightly-codeql-verification.json)
record durations and job IDs. This proves nightly/manual scanning works; the
full nightly run subsequently finished with the macOS integrity failure described below.

Trusted scoped-plugin warming is now observed: master run
[39064132902](https://github.com/obs-rust/obs-studio/actions/runs/39064132902)
saved `targets-ccache-linux-713ceb3b...`, a 25,761,103-byte archive on master.
The touched-target build and save steps succeeded. The prepared measurement
PR selects only `linux-v4l2`; its tracked compiler-input fingerprint matches
the seed revision, excluding Markdown and generated version metadata.
[Experiment records](ci-evidence/2026-10-17-plugin-cache-experiment.json)
retain the cold sample and trusted save. Subsequent PR compiler reuse is verified below; archive creation alone was
not used as proof of reuse.

### Scoped plugin follow-up measurements

| Condition | Compiler hits | Target step | Full job | Apt fetched |
| --- | --- | --- | --- | --- |
| Original cold PR | 0/41 | 57 s | 153 s | 147 MB / 13 s |
| Warm compiler archive | 41/41 | 70 s | 169 s | 147 MB / 30 s |
| Shared apt archive, wrong layout | 41/41 | 70 s | 190 s | 147 MB / 28 s |
| Shared apt archive, layout normalized | 41/41 | 46 s | 145 s | 142 MB / 13 s |
| Host-specific apt, cold | 41/41 | 57 s | 165 s | 147 MB / 21 s |
| Host-specific apt, warm | 41/41 | 39 s | 148 s | 0 bytes |

Compiler work fell from about 12 s to half a second, but this alone did not
make the first warm job faster. Package download/install overhead dominated.
The container archive supplied only about 5 MB of the host's package needs,
even after 563 archives were relocated correctly. The large shared restore
is therefore rejected. Merged #1335 instead uses a host-specific archive, with image rotation and
an enforced 200 MiB producer cap from #1339. Trusted master has now saved
147,542,391 bytes for image `20261012.163.1`. Its tracked compiler-input
fingerprint matches the prepared report PR. The cold host run retained 323
packages and took 165 s (57 s target step, 41/41 compiler hits). The subsequent
PR restored all 323 packages in 2 s and downloaded zero package bytes, with
41/41 compiler hits. Its target step took 39 s and full job 148 s: observed
reductions of 18 s (31.6%) and 17 s (10.3%) respectively versus the host cold
sample. Both used runner image `20261012.163.1` and the compiler inputs match
the trusted seed. This is one cold/warm pair, with network/runner variance;
it is not the representative-day queue-throughput result.
These are individual unmatched runs, with network and runner variation;
145 versus 153 s is not claimed as a robust causal improvement.

### Full nightly outcome

[Final job results](ci-evidence/2026-10-17-nightly-final-results.json) show
successful Ubuntu OFF/ON app builds, macOS Intel/Apple Silicon, Windows
x64/arm64, Flatpak, security analysis, Linux OFF/ON Tier 2 and Windows Tier 2.
The **overall run failed** at macOS `test_ingest_refresh_wait` /
`failed_refresh_ends_wait`: its elapsed time exceeded 150 ms. The first
implementation run failed, so the second implementation was not validated.
[Failure evidence](ci-evidence/2026-10-17-nightly-macos-failure.json) and
[#1334](https://github.com/obs-rust/obs-studio/issues/1334) preserve the failure
and required diagnostics. Scheduling sensitivity is a hypothesis; no cache
cause is established. The existing C test was neither weakened nor skipped,
and no speculative whole-workflow retry was performed. This native build
matrix is distinct from the AppImage nightly release pipeline.

The full Ubuntu app cache has a separate confirmed defect: 13 baseline
restore events unpacked 5–6 GiB and then deleted the directory at a 2 GiB
pre-build threshold. [#1337](https://github.com/obs-rust/obs-studio/pull/1337)
consumes the cache first and bounds the subsequent save instead. Its shell
regression covers 6 GiB, 8 GiB and over-limit snapshots; actual warm app reuse
still needs live verification. The 8 GiB guard is uncompressed per-run disk
state, not additional Actions storage allocation.

### Measurement status and limits

The before window is 2026-10-16 17:00 through 2026-10-17 17:00 UTC: 473 listed
runs and 117 detailed samples. Successful code PRs (n=9) had median gate wall
1,016 s; code merge groups (n=8) had median 954 s. Sampled canceled code PRs
used 211.57 runner-minutes and sampled canceled code merge groups 194.45.
These cancellation costs describe the sample, not an extrapolated population.
Docs/configuration cohorts are kept separate.

[Baseline log observations](ci-evidence/2026-10-16_17-ci-baseline-log-observations.json)
preserve the observed compiler counters, archive sizes, saved keys and test
counts. Logs were unavailable for 33 of 117 sampled attempts, all canceled;
missing counters are unknown. Repeated archive messages and independent
compiler scopes must not be summed into fictitious transfer/hit totals.

The [per-lane baseline](ci-evidence/2026-10-16_17-ci-baseline-lane-timings.json)
separates code/docs/configuration cohorts and records each literal step's
sample count. Selected **code-cohort successful jobs** were:

| Lane | PR n / median job | Queue n / median job | PR / queue median soldr setup |
| --- | --- | --- | --- |
| Fast Gate | 27 / 128 s | 19 / 139 s | 23 / 32 s |
| Tier 2 build | 8 / 196.5 s | 7 / 179 s | 25 / 17 s |
| Serial Tier 2 tests | 7 / 597 s | 3 / 598 s | Not applicable |
| Qt frontend | 5 / 256 s | 10 / 161 s | Not applicable |

These include successful jobs in workflows that later failed or were canceled;
the JSON discloses parent outcomes. They are not the successful-gate sample
counts above. Step medians are not additive totals, and historical lane/shard
names remain separate. Missing/skipped step times do not become zero samples.
The collector now emits `lane_cohorts` so the same method applies after rollout.

A [post-nightly inventory snapshot](ci-evidence/2026-10-17-post-nightly-cache-inventory.json)
records both cache APIs with collection timestamps. The itemized listing
contained 41 entries / 8,951,159,430 bytes; the usage endpoint reported 49 /
10,617,481,983 bytes. The pinned contract interprets `10GB` as 10 GiB
(10,737,418,240 bytes). These endpoints were asynchronous during concurrent
writers and janitor cleanup; their totals are not combined, and neither is
presented as steady-state headroom. An earlier listing peaked near 10.70 GB.
Nightly app archives dominate the transient footprint; useful queue caches
must remain warm through the full representative-day comparison.

### Confirmed retention pressure after rollout

[Capacity and eviction evidence](ci-evidence/2026-10-17-cache-retention-capacity.json)
records a confirmed janitor eviction at 20:12 UTC: the 711,970,692-byte Ubuntu
ON archive exceeded the shared `zccache` family's 1,782,579,200-byte allowance.
The same sweep removed the 1,083,740,873-byte Windows x64 app archive under
the shared Windows family's 838,860,800-byte allowance. These deletions are
distinct from the removed Ubuntu pre-build purge. The Ubuntu OFF archive's
disappearance remains unattributed.

Current family declarations sum to 16,951,069,900 bytes against the
10,737,418,240-byte repository budget. This sums shape ceilings and platform
cardinality; it is declared capacity, not simultaneous live usage. Increasing
nightly allowances alone cannot establish durable retention. The pinned
janitor protects only one newest entry per LRU family, so shared platform
families also allow independent consumers to compete for retention.

Before changing those bounds, allocate the whole budget from measured producer
sizes and reuse value, prioritize queue-critical caches, and evaluate separate
families and producer limits. Confirm that retained nightly seeds survive long
enough for a later consumer. Neither successful upload nor current headroom
proves that requirement.

The final after window must cover at least a full day of representative code
PR and queue traffic following rollout. It has not elapsed. The first host
cold/warm pair is complete. Repeatability over real traffic, Ubuntu app reuse
after the purge fix, the nightly integrity failure (#1334), stable cache
headroom and the representative-day comparison remain acceptance work under
#1286. This report does not claim those requirements are complete.

## Recommendation and acceptance checkpoint

Keep the merged queue policy, Rust correctness safeguard, bounded useful
compiler caches and parallel OFF/ON runners. Validate the observed host-package
benefit over real traffic; an archive HIT alone is not a throughput result.
Validate Ubuntu app reuse after removing the pre-build purge, and compare a
full day of code PR/queue traffic after the final cache rollout. Track the
macOS integrity failure separately through #1334 (proposed fix #1338).

| #1286 requirement | Evidence/status at this checkpoint |
| --- | --- |
| 1. Match cook profiles and measure consumers | Explicit profiles landed in #1318; local dev/release/no-cook experiment and native consumers recorded. Representative post-rollout comparison remains pending. |
| 2. Rust cache correctness | Reproduction still fails on pinned soldr; safeguard retained in #1290, upstream zccache#1943 linked. |
| 3. Remove unused overhead | Deferred Fast Gate layer removed in #1315; 13 Ubuntu restore/purge events fixed by #1337. Live Ubuntu reuse and budget behavior remain to verify. |
| 4. Warm scoped plugin builds | Trusted compiler and host-package seeds consumed by a PR: 41/41 compiler hits, 323 packages, zero package bytes downloaded; job 165 → 148 s in the first host cold/warm pair. Day-scale validation remains. |
| 5. Persist test timing | Nonempty OFF/ON producer, save, later restore and CTest consumption verified across master runs (#1288/#1311). |
| 6. Match remote save policy | All seven setup-soldr consumers use master-only remote saves; PR/queue/local-save behavior verified (#1291). |
| 7. Bound Qt cache | 1.2 GB compiler bound, 1.3 GB family, actual high hit rates and restore/save times verified (#1309). Overall repository headroom still needs the full-day view. |
| 8. Unify nightly cleanup | Competing purge removed; original startup permission failure fixed (#1289). Real full dispatch completed; separate macOS test failure remains #1334. |
| 9. Isolated libobs measurements | Three trials per condition and raw wall/CPU/cache evidence published in #1306. Wall-time ranges overlap. |
| 10. Shorten critical path | Independent OFF/ON runners landed (#1314); phase times, runner cost, skips and serial-test bottlenecks recorded. Overall queue gain remains unproven. |
| 11. Durable before/after comparison | Full-day baseline and cohort-separated per-step collector ready. At least one full representative post-rollout day remains outstanding. |

This checkpoint preserves the original scope. #1286 remains open while the
Ubuntu app reuse, stable budget behavior and representative-day
comparison are unverified. A green isolated job is not substituted for a
full workflow, and configuration/docs traffic is not substituted for code PRs.

## Isolated C libobs build

Measured the `libobs` target alone, with Rust libobs, frontend, scripting,
browser, websocket and unit-test builds disabled. Source revisions were
`be611227b18bc9df89ad089992e6f05eda6e53ba` (immediately before #1128) and
`d145635326a4753e729a4767eb233911258ef82b` (explicit unity/PCH groups).
Both used the same Nix dependency environment, GCC 15.2, RelWithDebInfo,
eight parallel jobs, and an AMD Ryzen 7 3700X. This is a local Linux build
experiment, not a GitHub runner measurement or an application benchmark.

| Condition | Before median wall (range), s | Current median wall (range), s | Before/current median CPU, s |
| --- | --- | --- | --- |
| Compiler cache disabled, 3 trials | 9.749 (9.654–9.899) | 9.407 (9.390–9.932) | 63.482 / 46.620 |
| Warm ccache rebuild, 3 trials | 1.543 (1.541–1.556) | 1.435 (1.428–1.447) | 2.652 / 2.368 |

The uncached wall medians differ by 3.5%, with overlapping ranges; this
experiment does not establish a robust wall-time gain. CPU work fell 26.6%.
The warm wall difference is 0.108 s (7.0%). Neither result supports calling
the broader 315 → 198 s benchmark a libobs-only improvement. The revisions
also contain intervening source changes, so this comparison measures the
two actual revisions rather than isolating unity as the sole cause. Load
averages are recorded per trial; no attempt was made to monopolize the host.

With ccache 4.13.6, every measured warm trial had 125/125 cacheable hits
before and 39/39 now. The initial cache-population build is retained in the
raw data but excluded from warm medians. Warm trials remove compiler
outputs before building, so they are real rebuilds rather than Ninja
no-ops. Current PCH outputs cover `libobs`, `libobs-objects`,
`libobs-core-modules`, and `libobs-version`; the remaining group families
include their own headers textually under the build policy.

[Raw trials, compiler settings, environment and cache statistics](ci-evidence/libobs-2026-10-17/)
are committed with the report. Configure and Nix environment startup time
are excluded from build timings. Existing application build directories and
global caches were preserved; benchmark outputs and per-revision ccache
stores live under the same checkout's ignored `build_ci_audit/` directory.

### Reproduce

Use the same checkout, switching source revisions between experiments.
Save the measurement helper under `build_ci_audit/` before checking out an
older revision that predates the helper. Run configure and measurement
inside the same dependency environment, for example
`nix-shell rust/tools/sanitize/shell.nix` on this Linux host.

```sh
mkdir -p build_ci_audit
cp ci/benchmark_libobs.py build_ci_audit/benchmark_libobs.py
# Switch to the revision being measured; use a new directory per revision.
cmake -S . -B build_ci_audit/libobs-example -G Ninja \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo -DOBS_BUILD_NUMBER=1 \
  -DENABLE_FRONTEND=OFF -DENABLE_SCRIPTING=OFF -DENABLE_BROWSER=OFF \
  -DENABLE_WEBSOCKET=OFF -DENABLE_RUST_LIBOBS=OFF -DENABLE_UNIT_TESTS=OFF \
  -DENABLE_AJA=OFF -DENABLE_WEBRTC=OFF -DENABLE_QSV11=OFF \
  -DENABLE_VST=OFF -DENABLE_DECKLINK=OFF \
  -DENABLE_ZCCACHE=OFF -DENABLE_CCACHE=OFF
uv run build_ci_audit/benchmark_libobs.py \
  --build build_ci_audit/libobs-example --output build_ci_audit/example-cold \
  --repetitions 3 --jobs 8 --rebuild
```

For warm-cache trials, add ccache to the environment, reconfigure the same
dedicated build with `-DCMAKE_C_COMPILER_LAUNCHER=ccache` and
`-DCMAKE_CXX_COMPILER_LAUNCHER=ccache`, set a fresh per-revision `CCACHE_DIR`
under `build_ci_audit/` and `CCACHE_MAXSIZE=250M`, then run the helper with
`--repetitions 4 --rebuild --cache-stats`. The first trial populates the
empty cache; compare the next three. The helper requires `safe-rm` to move
only the dedicated build's compiler outputs to trash before each trial.

## Rust compiler cache: verified blocker

The pinned soldr 0.9.35 still reproduces
[zccache#1943](https://github.com/zackees/zccache/issues/1943). A nested Rust
module changed from a comment to `compile_error!`, preserving its size and
nanosecond mtime. Touching the unchanged crate root forced Cargo to invoke
rustc. The cached command returned success with a HIT; the same command
with `SOLDR_RUSTC_WRAPPER=none` failed with the injected diagnostic.

| Step | Exit | Probe error reported | Seconds |
| --- | ---: | --- | ---: |
| Warm valid source | 0 | No | 2.888 |
| Cached changed source | 0 | No: false success | 2.052 |
| Uncached changed source | 101 | Yes | 1.621 |

These durations describe a tiny correctness fixture, not a performance
benchmark. [Captured result](ci-evidence/2026-10-17-rust-cache-source-probe.json).
Keep the local/CI gate's wrapper bypass until this reproduction passes on
the version we adopt. The standalone Linux C/C++ PCH cache upgrade in
[#1265](https://github.com/obs-rust/obs-studio/pull/1265) is separate from
soldr's embedded Rust cache and does not prove this issue fixed.

Reproduce from the repository root with the same executable CI pins:

```sh
uv run ci/cache_source_probe.py \
  --directory build_ci_audit/source-probe-new \
  --soldr /absolute/path/to/soldr --expected-version 0.9.35
```

Use a new scratch directory for every run. The probe preserves logs and
`result.json`, and exits 1 when the cache fails the source-safety check.
The explicit binary and expected-version check matter: in this checkout,
`uv run` found soldr 0.9.34 on its environment PATH while the shell's soldr
was 0.9.35. Both reproduced the stale success; the table records the explicit
0.9.35 run. No cache or daemon was purged to obtain these results.

The required gate's existing source probe now exercises a nested module
edit preserving size and mtime, so it guards the workaround against this
specific failure mode rather than checking only an ordinary append.

## Timing persistence and scheduled retention

[#1288](https://github.com/obs-rust/obs-studio/pull/1288) enables hidden-file
uploads for `.tier2-cost` and rejects missing or unmeasured OFF/ON timing
files before writing a cache. #1311 subsequently fixed the singleton-artifact
download layout. Master run 39059448618 produced 282 measured records per
mode and saved a 5,920-byte timing cache. Later run 39060831338 restored the
newer nonempty timing entry and completed both CTest modes. Producer,
persistence and consumer evidence are recorded in #1286.

The [failed scheduled run](https://github.com/obs-rust/obs-studio/actions/runs/39002695404)
was rejected before job execution: its annotation says the nested `ci-mode`
job requested `actions: read` but the caller allowed none. This was a workflow
permission error, not a cache script failure.
[#1289](https://github.com/obs-rust/obs-studio/pull/1289) grants that permission
and removes the independent nightly compiler-cache purge. Cache retention
belongs to the existing merge-group/master janitor and `ci.toml`; scheduled
builds reuse its warm entries. Dispatch 39064183327 subsequently started every
job and completed the full native app matrix and security scans. Its separate
macOS ingest-refresh assertion failure (#1334) remains visible; it is not a
startup or cleanup failure.

## A day of baseline CI traffic

The window 2026-10-16 17:00 → 2026-10-17 17:00 UTC contains 473 Pull/Push
workflow runs. Both workflow listings extend before the start of the window,
so the listing limit did not truncate it. Detailed jobs and actual changed
files were inspected for 117 runs, sampled across event/outcome and time,
with extra documentation candidates whose diffs were verified. These are
sample medians, not weighted population estimates.

| Successful gate cohort | Samples | Median created-to-gate time |
| --- | ---: | ---: |
| Code PR | 9 | 16m 56s |
| Code merge group | 8 | 15m 54s |
| Docs PR | 1 | 4m 01s |
| Docs merge group | 2 | 6m 58s |
| CI/config merge group | 2 | 2m 55s |

The sparse docs samples do not establish a stable typical duration.
Across the full listing, merge groups had 35 successful, 163 canceled and
6 failed runs. The sampled canceled code merge groups alone consumed
194.45 runner-minutes. That is observed sampled waste, not an extrapolated
total, and the run outcomes alone do not establish why cancellation occurred.
The window spans earlier CI/build rollouts; this baseline cannot attribute
a later difference to any single cache or concurrency change.

[Summary, outcome counts, listing coverage and per-lane medians](ci-evidence/2026-10-16_17-ci-baseline-summary.json)
and [sample IDs, diffs, gate and runner times](ci-evidence/2026-10-16_17-ci-baseline-samples.json)
are retained. Cache inventory in the summary is a snapshot at collection
time, not a reconstruction of cache contents at the historical window end.
The full raw run/job/cache responses remain under `build_ci_audit/metrics-before/`
in the measurement checkout; the collector reproduces that evidence.

`ci/cache_metrics.py` resolves source cohorts from actual compare diffs,
including an unambiguous PR branch/lifetime fallback for older runs whose
API metadata no longer contains a PR association. Missing/ambiguous diffs
remain unknown. It reports latest run attempts only and does not reconstruct
overwritten attempts. In addition, every merge-group CI Gate summary now
lists completed setup/cache step durations from the already-fetched job
metadata, without adding another API request to the gate.

Post-change measurements must use a separate window and preserve these
cohort/coverage distinctions. The day-long post-change comparison remains
open until the configuration changes have landed and that window exists.

## Unused gate compiler-cache layer

Master run [39056485478](https://github.com/obs-rust/obs-studio/actions/runs/39056485478),
Fast Gate job 117465447413, restored a 1.2 KB build-cache archive (four
files, 1.8 KB inflated) in 0.7 s. Its final session counters were zero
hits, misses and compilations; post-job save skipped the zero-compile
delta. The local gate explicitly bypasses this Rust compiler cache for
lint, tests and cross checks because of the verified correctness blocker.

Merged #1315 disables this layer only for deferred PR/merge-group Fast Gate runs,
retaining cook and registry snapshots. The full-day sample contains 26
PR and 18 merge-group Fast Gate session reports with no positive compiler
hits (26 and 16 respectively had zero compilations). In contrast, 5/24
master Fast Gate samples and 3/8 PR/queue cross-lint samples had positive
hits. Keep full/master and cross-lint caching rather than extrapolating
from the initial zero-counter run.

The Rust wrapper bypass does not establish that every compiler-cache
consumer is inactive: dependency cooking and C compiler work may still
use it. [Captured sample coverage](ci-evidence/2026-10-17-gate-cache-consumption.json).
This is a small measured operation to remove from the deferred path, not
a claim of a large CI gain. Cold cook may lose potential compiler-cache
reuse after a lock change; retain the published post-job cook snapshots
and measure total live setup/consumer time. Concurrent action phase
times must not be summed as sequential wall time.

## Additional cache inventory: CodeQL overlays

The cache API snapshot at 2026-10-17 18:39 UTC contained 43 entries totaling
4,389,323,635 bytes (below the 10 GB repository budget). This is a live
snapshot, not historical baseline occupancy. Qt's newest entry was
1,114,881,014 bytes; #1309 subsequently corrected its bound and allowance.

At that initial snapshot, GitHub's default CodeQL setup wrote
`codeql-overlay-base-database-*` entries outside the explicit workflow files.
The snapshot included a 131,294,279-byte C++ base and Python bases around
11 MB. They were undeclared then. PR run 39057119695, C++ job 117467259478,
restored the master overlay in about 3.7 s; master run 39057995138 saved a
new base in about 5.1 s. This demonstrated consumption, not a measured
analysis speedup against a cold run.

#1316 subsequently declared a bounded 200 MB LRU overlay family. #1327 then
disabled automatic default setup and moved all four languages to the existing
nightly workflow. The explicit nightly C++ job 117488229574 still uploaded
an overlay base, so the bounded family remains relevant after that migration.
Security analysis stays outside PR/merge-group admission. Its cache behavior
is independent of the compiler-cache hit rates used to assess build speed.

## Queue publication and deeper unit-test caching

The proposed single publisher must account for GitHub's branch-scoped caches:
temporary merge-group caches are not shared default-branch seeds. A queue
payload could be promoted by an existing master-push workflow after verifying
the successful queue run and matching source tree. That publisher would be
the cache-write exception; ordinary jobs could enforce `cache-mode: read`.
No publication-policy change or promotion implementation is claimed here.
GitHub caches are immutable, so refreshed payloads need generation keys.

Deep Rust state is already present: the pinned setup action's native macOS
post-step saved a `full` archive with 8,238 files, and the local dev archive
contains test fingerprints and incremental state. With the pinned action's
default `cook-delta: false`, a warm base selects no save layer. Measure bounded
generation refresh and consumption before adding a duplicate target archive.

The existing Tier 2 execution-tree artifact is 177,234,558 bytes in run
39069670404. It contains binaries/runtime files, excludes objects and Cargo
state, and currently transfers work from the build job to OFF/ON test jobs.
Exact-input reuse could save compilation work; it cannot replace executing
tests, and it does not supply changed-source incremental reuse by itself.

[A manual deep-cache probe](ci-evidence/2026-10-17-deep-test-cache-probe.json)
demonstrates a separate invalidation problem with the compiler wrapper disabled:

| Condition after target restore | Observed result |
| --- | --- |
| Unchanged source | Unit-test binary reused and executed successfully |
| Ordinary nested edit adding `compile_error!` | Recompiled; exit 101 with current diagnostic |
| Same edit preserving size and nanosecond mtime | Old unit-test binary executed; false success |
| Invalidate changed package's fingerprints | Recompiled; exit 101; unchanged dependency stayed cached |

The guard quarantines package fingerprints without deleting the cached
dependency. The fixture knows which package changed; automatic content-to-package
mapping, generated/build-script inputs and full-workspace invalidation remain
unimplemented. This validates a mechanism, not production cache safety or a
performance improvement. Preserve both existing safeguards while evaluating it.

Run the probe in a new directory within this checkout:

```sh
uv run ci/deep_test_cache_probe.py \
  --directory build_ci_audit/deep-cache-new \
  --soldr /absolute/path/to/soldr --verify-guard
```

`--verify-guard` requires unchanged reuse, ordinary-edit rejection, guarded
same-stamp rejection and dependency preservation. Without that flag the probe
exits 1 when it reproduces stale unit-test success. Logs and the archive remain
in the chosen directory. No global cache or daemon is purged.

### Build-script inputs can defeat deep reuse

The workspace-shaped delta experiment produced a 62,371,228-byte delta versus
a 188,072,837-byte updated full snapshot. Both restores nevertheless rebuilt
`obs-util`. Cargo's diagnostic identified the same source path changing from
1792262706.251336821 to 1792262706.251000000 seconds after hydration. Its
default build-script package scan treated the lost sub-millisecond precision
as changed input. Restoring that one exact original timestamp eliminated the
rebuild in a diagnostic control; no source bytes changed.

The downstream mitigation declares the actual `obs-util/build.rs` inputs:
itself and the applicable Windows C shims. Rust sources remain rustc inputs.
After rebuilding and saving this configuration, an unchanged full restore
kept test products fresh without timestamp edits: compile-only command
0.816 s, versus 6.181 s before the mitigation in the earlier diagnostic run.
These are individual local observations, excluding network and test execution;
the new snapshot's save/restore still cost 12.620/16.628 s. No whole-CI gain is
claimed. [Raw results](ci-evidence/2026-10-17-obs-util-build-inputs.json).

The upstream replay-precision issue remains
[soldr#3851](https://github.com/zackees/soldr/issues/3851). This mitigation
does not address same-stat source edits or justify removing the correctness
safeguards. Native Windows input tracking still requires CI validation.

## Post-job cook-profile comparison

The pinned action saves its cook archive **after** the job's consumers, so
an archive initially cooked with `--release` can contain dev/test artifacts. A pre-consumer
snapshot is not an adequate model for disabling this cache.

The isolated local experiment used soldr 0.9.35, four Cargo jobs, identical
compiled Rust inputs and dependency environment, wrapper bypass enabled
for consumers, and post-job snapshots taken after workspace clippy and
`obs-util` tests. Each restored snapshot ran warm cook, workspace clippy
and the same tests. Policy order rotated across three trials per scenario.
A forced core rebuild touched the unchanged obs-util crate root and
required actual recompilation in the log; source bytes were preserved.

| Policy | Unchanged median (range), s | Core rebuild median (range), s | Archive bytes |
| --- | ---: | ---: | ---: |
| release cook | 57.259 (56.007–57.611) | 67.869 (67.801–90.592) | 205,118,438 |
| dev cook | 36.170 (35.849–40.269) | 82.736 (48.692–139.906) | 187,723,562 |
| no cook/archive | 72.136 (71.493–74.113) | 78.291 (73.830–79.011) | 0 |

[Raw trial results](ci-evidence/2026-10-17-soldr-cook-post-job.json) include
individual command, restore and warm-cook durations and load averages.
The totals include local restore plus consumer commands, excluding network
transfer, formatters, Dylint and post-job save. Initial archive creation
took 8.534 s for release and 7.203 s for dev. Local archives cover selected
tests and are smaller than the 637,542,999-byte live CI release snapshot;
their sizes are not predictions of full-workspace CI archives.

Dev cooking clearly wins the unchanged-input scenario here. Core rebuild
variance is large and the ranges overlap; its median favors release, so
these data do not establish a universally fastest profile. Disabling cook
loses useful post-job artifact reuse. Retain release cooking on core work
until live comparisons justify changing it, and evaluate dev cooking for
non-core Fast Gate runs with unchanged workspace inputs. Match explicit
release/static-library consumers to release; do not infer cross-target
benefits from this host-only experiment.

Reproduce using a fresh directory in the same checkout and the same Nix
dependency environment (existing target caches are left intact):

```sh
nix-shell rust/tools/sanitize/shell.nix --run \
  'uv run ci/benchmark_soldr_cache.py --directory build_ci_audit/cook-new --soldr /absolute/path/to/soldr --trials 3 --jobs 4'
```

Keep compiled sources fixed throughout. The recorded run reused completed
seed consumers at their original target paths, then modeled actual
post-job saves. A measurement assertion initially expected Cargo to label
a forced rebuild only “Checking”; Cargo correctly used “Compiling”. The
assertion was corrected, that attempt's logs were preserved, and only the
remaining unrecorded trials resumed. All published trials passed.


## Dylint setup-soldr feature audit

Fast Gate currently invokes `soldr cargo dylint` after stable-profile
clippy. In master run 39056485478, job 117465447413, catalogued Dylint
tools/driver downloaded from 18:05:40.654 to 18:05:41.944. Nightly
`rustc-dev`, `rust-src` and `llvm-tools` preparation then lasted until
18:06:06.119, roughly 24 seconds. The repo plugin pins
`nightly-2026-05-28`; caching that foundation is a plausible opportunity.

However, pinned setup-soldr b782f606a2f3beadc35b9e3bf5b4565e005f52ed
[resolve-setup.ts](https://github.com/zackees/setup-soldr/blob/b782f606a2f3beadc35b9e3bf5b4565e005f52ed/src/lib/resolve-setup.ts)
forcibly sets `inputs.prebuildDeps = "none"` whenever `dylint: true`,
including explicit cook inputs. That mode assumes a Dylint-only job; it
is not a drop-in improvement for this mixed stable-check/test/Dylint job.
Its foundation cache also owns exact nightly paths, while legacy
`dylint-cache: true` alone does not include those paths by default.

**Decision:** retain the current mixed-job mode and ordinary cook reuse.
Do not enable `dylint: true` indiscriminately to maximize feature count.
The measured post-job cook benefit would be lost, and the network/restore
tradeoff of a separate foundation has not yet been measured. A dedicated
Dylint lane or an explicit mixed-job foundation implementation would need
its own setup/restore/consumer/save benchmark, required-gate integration,
nightly/target identity checks and bounded retention before adoption.
No workflow or cache family was added for this rejected rollout.

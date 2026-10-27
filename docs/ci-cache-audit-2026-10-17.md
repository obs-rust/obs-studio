# CI cache audit: 2026-10-17

> The `docs/ci-evidence/` snapshots linked below are retired from `master`
> (#2686); each link resolves to the commit that last held the file
> (`c2af3f9c3`), and the commands quoted below name those paths as they were
> then.

Implementation and measurement tracker:
[obs-studio#1286](https://github.com/obs-rust/obs-studio/issues/1286).

## Current executive verdict — October 18 UTC

**The latest full native validation is successful.** Run
[39089156757](https://github.com/obs-rust/obs-studio/actions/runs/39089156757)
at `df86995022` passed all seven app builds: Ubuntu OFF/ON, Windows x64/arm64,
macOS x86/arm64 and Flatpak. Native Rust tests, Tier 2 and nightly scans also
passed. Linux OFF/ON each registered 296 tests with zero failures; CTest took
314.80 / 304.42 s. Both Ubuntu modes published compatible snapshots.
[Current full-run and producer evidence](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-ubuntu-compatible-producer.json).
The earlier full run at `2f9bf8d77` also passed; its distinct 295-test results
remain [recorded separately](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-full-warm-validation.json).

| Finding | Verified result | Recommendation/status |
| --- | --- | --- |
| Native compiler reuse | Linux Tier 2 784/785 cached; Windows Tier 2 308/309 cacheable hits | Preserve native compiler stores |
| Warm Ubuntu ON | Full job 684 → 435 s; build 602 → 324 s, in unmatched observations | Keep the working ON seed; measure durability |
| Ubuntu OFF fallback | 43/361 calls cached; mixed state 9,526 MiB; save skipped | #1371 verified: later OFF restored the exact seed and achieved 208/345 cacheable hits; remaining misses and retention unresolved |
| Embedded Rust store | All eight expanded soldr lanes reported no embedded session | #1372 disables transport; #1380 retired retention and the queue janitor reclaimed 183,330,575 bytes |
| Compiler correctness | Rust stale-source/target blockers remain; a separate rapid-edit native C probe also returned stale output | Keep Rust bypass; follow native freshness upstream #1935 and avoid deeper reuse without validation |
| Startup incident | Local reproduction plus PR, queue, master and full-run verification | #1365 verified; #1355 closed |
| Storage headroom | Post-run listing summed to 12.43 GB; separate usage API disagrees; useful plugin seeds later absent | Resolve global-budget policy and preserve queue-useful seeds; no stable-headroom claim |
| Representative queue throughput | Product-code queue observation 843 s; older unmatched median 954 s | Full post-rollout day still required; no causal speedup claim |

**Build improvements are real but scoped.** The isolated libobs experiment
measured median CPU 63.482 → 46.620 s (26.6% lower); uncached wall
9.749 → 9.407 s had overlapping ranges. The broader app's historical
315 → 198 s result is a different benchmark. Neither establishes the
current merge queue's end-to-end improvement.

![Isolated libobs trials showing CPU medians 63.482 to 46.620 seconds and overlapping uncached wall-time ranges](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/libobs-2026-10-17/build-trials.svg)

The plot shows all three trials and min–max ranges. Its warm panel excludes
the initial seed build and uses a separately identified grouped revision.
[Raw trials and reproducible plotting script](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/libobs-2026-10-17/plot_trials.py).

**Next priorities:** investigate the remaining Ubuntu OFF misses,
resolve the independently reproduced recording-wait race blocking #1362,
restore durable retention of the measured plugin seeds, and collect at least
one representative post-rollout day. Qt publication
reuse and skipped upload are verified for one docs-only pair. The proposed
queue-only publisher needs a transport compatible with temporary queue refs;
cross-run artifacts are feasible, but that architecture has not been deployed.

## Retention verification after the full run

PR #1380 merged after successful queue run
[39088853334](https://github.com/obs-rust/obs-studio/actions/runs/39088853334).
All seven setup-soldr callsites already disabled embedded compiler-store
transport; retiring its unused family allowed the actual queue janitor to
delete all three remaining archives, totaling **183,330,575 bytes**. A fresh
inventory contained zero such archives. The pinned offline executor selected
exactly those three additional IDs, leaving its other decisions unchanged.
[Executor and runtime evidence](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-retired-embedded-cache.json).
This is verified storage reclamation, not a measured CI speedup or proof of
stable headroom.

The subsequent master run 39088999741 passed. Its Tier 2 build restored
compiler, apt and timing snapshots and reported 785 cached / 1 cold native
compilations; OFF and ON each registered 296 tests with zero failures.
[Post-retirement master observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-retired-embedded-master-validation.json)
verify useful native reuse survived this change. A single passing run does
not establish durable retention or resolve the separate recording race.

The useful touched-plugin compiler and apt seeds were present in the saved
00:32 UTC inventory (28,117,319 and 147,539,809 bytes), but absent in the
01:56 inventory. Seventeen completed janitor logs inspected from 00:42 to
01:55 record no deletion of those IDs. These observations establish a retention
gap; they do not identify the deletion actor or prove GitHub eviction.
[Entries, inspected jobs and limitations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-plugin-cache-retention-gap.json).
The earlier 41/41-hit warm consumer remains valid historical evidence, while
retention durability remains unresolved under #1269.

A later master run, 39088389865, failed the independent recording-wait
assertions in ON (296 registered CTests, one failing executable); OFF passed
all 296. The successful full native run above predates this recurrence.
The proposed helper patch has not been applied, and no tests were skipped or
relaxed. See [the #1283 recurrence](https://github.com/obs-rust/obs-studio/issues/1283#issuecomment-6244581047).

### Post-compatibility Ubuntu producer validation

In full run 39089156757 on `df86995022`, Ubuntu ON passed with 340 cached,
5 cold and 16 noncacheable compiler calls. It restored the compatible ON
archive, then saved a new 795,699,586-byte archive from 5,488 MiB of raw
state, below the 8,192 MiB guard. Restore/build/save took 19/234/18 s; the
whole job took 340 s, including successful GPU and app Tier 4 checks.
[Producer metadata](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-ubuntu-compatible-producer.json)
keeps this separate from the earlier unmatched warm observations.

OFF also passed. It found no compatible archive, built all 345 cacheable
calls cold (16 other calls noncacheable), and published its own snapshot:
5,045 MiB raw, below the 8,192 MiB cap. The API confirmed both mode-specific
seeds were present after the jobs completed. This verifies producer behavior
under #1371, replacing the previous mixed-mode 9,526 MiB save failure. A later
OFF consumer is recorded below; stable retention remains unverified. The parent full run
subsequently passed all seven native app builds and the remaining checks.


The two new Ubuntu archives total 1,508,547,016 bytes, exceeding their pooled
zccache family footprint of 1,153,433,600 bytes (shared with AppImage and
sanitizer entries). The pinned janitor, run offline on the saved inventory
with a hypothetical clock beyond the grace window, plans deletion of the
ON archive and retention of OFF. Both were present in the real inventory;
this is a predicted future plan, not an observed deletion.
[Exact entries, capacity and dry-run plan](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-ubuntu-retention-plan.json).
Mode-compatible restores repair correctness of reuse and snapshot sizing,
but cannot keep both modes warm under this allocation. Whole-budget policy
remains unresolved under #1269; no global allocation change is claimed here.

### Same-revision Ubuntu consumer results

Both Ubuntu jobs passed in consumer run 39090099191, at the same source
revision as the producer, and restored their exact new mode-specific keys.

| Mode | Producer cached / cold | Consumer cached / cold | Build, producer → consumer | Whole job, producer → consumer |
| --- | --- | --- | --- | --- |
| OFF | 0 / 345 | 208 / 137 | 630 → 359 s | 661 → 405 s |
| ON | 340 / 5 | 343 / 2 | 234 → 212 s | 340 → 324 s |

Each job also reported 16 noncacheable calls. These are individual runner
observations, not a controlled hardware comparison or queue-throughput result.
[Consumer logs, counters and step timing metadata](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-ubuntu-compatible-consumer.json).
The parent consumer workflow subsequently passed, including all seven native
app builds and the remaining checks.

OFF's runner image, GNU 15.2.0, zccache 1.15.5 and all 306 logged installed
package versions match the producer. That excludes the visible version
differences; it does not explain the 137 cold calls. These logs lack
per-compilation hit/miss records, so neither their affected units nor root
cause are established. Keep this gap distinct from the now-verified cache
transport and compatibility fix.

The save actions still compressed the stores on exact hits, taking 14 s OFF
and 11 s ON, then warned that the immutable keys could not be reserved.
Merged #1381 adds a guard to skip this redundant work; subsequent Ubuntu
execution of that guard is not yet a verified runtime saving. Restore, build and integrity coverage remain required.

### Native journal diagnostic and correctness limit

Pinned zccache 1.15.5 supports private sessions with `--stats --journal
<path>.jsonl --profile`, followed by offline `zccache analyze --json`. Raw
journal records contain arguments, outcomes, `miss_reason` and `miss_diff`.
This provides a diagnostic route beyond the current CI aggregate counters;
no journal instrumentation has been enabled in production CI by this audit.

An isolated GCC 15.2.0 fixture exposed a separate freshness failure: compile a
function returning 7, warm it, then immediately write return 8 with equal
size and a naturally changed nanosecond mtime. The journal reports a hit and
the cached executable returns 7; a fresh compile returns 8. The unwrapped GCC
binary reproduces the result, so it is not limited to the Nix compiler wrapper.
A separate 200 ms settling control returns the correct value; the delay is
not a production workaround. Private cache/endpoint ownership keeps these
fixtures separate from the shared daemon.
[Source, executable results, stamps and raw journals](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-native-journal-freshness.json)
corroborate existing upstream [zccache#1935](https://github.com/zackees/zccache/issues/1935#issuecomment-6244827358).
This does not establish wrong output in the GitHub runs, explain OFF's 137
misses, or identify a specific internal shortcut from aggregate outcomes.
The compiler-cache correctness recommendation now includes this native
rapid-edit limit alongside the existing Rust and restored-target blockers.

Upstream 1.15.6 contains a separate Clang PCH timestamp-isolation fix
([#1958](https://github.com/zackees/zccache/pull/1958)). Neither that release
nor a production version bump is qualified by the measurements here; it is
not asserted to fix this GNU fixture or the observed CI misses.

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

[Machine-readable rollout results](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-master-cache-rollout.json)
include job IDs, phase times and independent compiler counters. The isolated
libobs results below remain separate from whole-app and CI build timings.

### Full native validation and cache survival, 22:15 UTC

Full manual run [39074304012](https://github.com/obs-rust/obs-studio/actions/runs/39074304012)
at `d315ee9c6` passed all seven native app builds: Ubuntu OFF/ON, Windows
x64/arm64, macOS x86/arm64 and Flatpak. This confirms native unity/PCH build
success at that revision. The overall workflow **failed**: Linux OFF
`test_slideshow_empty` returned `obs_startup=false` in group setup. The startup failure is tracked in
[#1355](https://github.com/obs-rust/obs-studio/issues/1355). Subsequent local
reproduction and the merged Xvfb lifetime fix are described below; that does
not establish the cause of every historical startup incident.
The X-display informational message also occurs in passing tests and does not
diagnose that failure. No test assertion was relaxed.

The Tier 2 build recorded 779 cached compilations out of 780. Its execution
overlapped Fast Gate by 142 seconds and finished 57 seconds before the gate.
This verifies actual compile overlap, not a representative queue speedup.
Both Ubuntu app builds were cold (345 misses, 16 noncacheable calls each),
retained their 5,045 MiB raw compiler state through the post-build size guard,
and saved OFF/ON archives of 712,826,136 / 712,780,180 bytes. A subsequent
warm consumer is still required to establish their value.

Cache survival remains unresolved. The Tier 2 job restored the earlier seed
and saved a refreshed `d315ee9c6` archive at 22:19:04 UTC; the 22:34 janitor
inventory had no Tier 2 family. The PR janitors inspected in that interval
were skipped. These observations do not identify who removed the archives or
prove global-capacity eviction. Separately, the 22:36 janitor explicitly
deleted the Ubuntu OFF archive under the pooled nightly LRU budget, retaining
ON. The family split protects Tier 2 from that pool's eviction decisions;
it does not yet demonstrate durable retention.

The loss affected a real consumer: PR #1353's Tier 2 build
[job 117521595282](https://github.com/obs-rust/obs-studio/actions/runs/39075342558/job/117521595282)
missed all Tier 2 compiler-cache restore keys at 22:33:13 UTC, then reported
782 total compilations, zero cached and 782 cold. The successful job took
426 seconds, including a 345-second build step. Its apt, soldr cook/build
and CTest timing restores succeeded. This establishes an archive miss and
cold compilation, but changed source and runner conditions prevent a matched
runtime attribution. The disappearance therefore has an observed consumer
cost even though its cause remains unresolved.

[Job identities, times and observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-full-validation.json)
keep the successful native builds, failed test setup, cold cache production
and unresolved cache loss separate. The full-day comparison remains open.

### Startup fix verified in the merge queue, 00:10 UTC October 18

PR [#1365](https://github.com/obs-rust/obs-studio/pull/1365) merged at
00:10:16 UTC after queue run
[39081091301](https://github.com/obs-rust/obs-studio/actions/runs/39081091301)
succeeded. Both implementations registered 295 tests with zero failures;
CTest took 314.95 s OFF and 308.87 s ON. The slideshow test and the new
startup-failure diagnostic test passed in both modes. These observations
verify queue coverage of the Xvfb `-noreset` fix; they do not replace the
pending full nightly run or establish a throughput improvement.

[Machine-readable queue results](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-startup-queue-validation.json)
keep this verification separate from the earlier PR results and local
repeated-startup experiment.

Linux release setup also now uses `linux-host-v1`: PR
[#1367](https://github.com/obs-rust/obs-studio/pull/1367) merged at 00:11:16 UTC.
All seven setup-soldr callsites now have explicit consumer profiles, and all
Linux callsites distinguish host and container cache identities. The release
workflow itself was not dispatched; its subsequent warm-consumer performance
remains unmeasured.

### Master validation and controlled recording-wait reproduction

Master run [39081770788](https://github.com/obs-rust/obs-studio/actions/runs/39081770788)
passed after the startup fix: both modes registered 295 tests with zero
failures; CTest took 314.72 s OFF and 308.66 s ON. Both the startup diagnostic
and recording-shutdown test passed. [Raw master observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-startup-master-validation.json)
do not disprove the recording race seen in a different run.

The unchanged recording-shutdown test passed 20 local repetitions. A separate
minimal fixture using the production helper, a real signal handler and an
atomic active flag demonstrated the inspected race: after `deactivate`, a
controlled 100 ms worker pause before the inactive store lets the UI helper
return `stopped=true` while `active=true`. Both signals were registered and
the worker joined cleanly. This confirms a possible ordering failure under
controlled scheduling; it does not prove that pause occurred in historical
CI. [Fixture source, source identities and results](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-recording-wait-race-probe.json)
are retained separately from the unchanged-test results. The follow-up is
posted to the existing #1283 claim; production code and existing tests were
not edited.

### Warm Ubuntu consumers and full-run write pressure

Full run [39082733551](https://github.com/obs-rust/obs-studio/actions/runs/39082733551)
completed successfully, including both Ubuntu jobs. Both restored the earlier ON archive (712,780,180 bytes).

| Mode | Compiler results | Restore | Build, cold → warm | Job, cold → warm |
| --- | --- | --- | --- | --- |
| ON | 200 cached / 145 cold / 16 noncacheable | 23 s | 602 → 324 s | 684 → 435 s |
| OFF, using ON fallback | 43 cached / 302 cold / 16 noncacheable | 28 s | 584 → 574 s | 629 → 619 s |

These are individual unmatched observations across revisions and runners,
not causal benchmarks. ON passed both Tier 4 checks and saved a 794,953,805-byte
archive. OFF accumulated 9,526 MiB of mixed compiler state, exceeded the
8,192 MiB save guard and published no OFF archive. The opposite-mode fallback
therefore provided limited observed reuse and prevented seeding this mode in
this run. Restore compatibility and snapshot growth must be addressed before
claiming both modes are warm. [Raw observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-ubuntu-warm-consumers.json).

A mid-run listing summed to 12,218,332,982 bytes, above the declared 10 GiB
budget. Three Qt snapshots alone held approximately 3.48 GB; their observed
jobs each had only 2–4 compiler misses. The separate usage endpoint reported
8,389,095,940 bytes, so these are different observations, not proof of an
atomic storage peak or a particular eviction actor. Avoiding redundant
per-commit Qt snapshots is a remaining opportunity; preserve useful seeds.

Fast Gate also missed its host release cook key, then received HTTP 409
on reservation. Two later listings had no visible entry for that exact key.
The cause remains unknown; it is tracked in
[setup-soldr#581](https://github.com/zackees/setup-soldr/issues/581).
[Inventory summaries and explicit limits](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-cache-pressure-and-cook-conflict.json)
keep the publication conflict separate from compiler hits and storage totals.

### Merged transport policy and a post-native-burst consumer

PR #1372 merged at 00:49:59 UTC. Its successful queue run
[39084053608](https://github.com/obs-rust/obs-studio/actions/runs/39084053608)
reports the embedded build-cache layer as disabled while restoring the exact
host cook archive. The action's policy layers are now toolchain, registry
and cook; setup took 19.1 s in this configuration cohort. This verifies the
transport choice without claiming a product-code speedup.

A later master consumer, job 117546797737 in run 39083903921, restored the
full native run's `2f9bf8d77` Tier 2 seed and recorded 784/785 cached
compilations, then saved a new seed. The later inventory retained that new
seed. Thus the queue compiler store has observed useful consumption after
the native write burst. Registry and mini-tool cache misses still occurred;
this single observation does not establish durable overall retention.
[Runtime policy and consumer evidence](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-cache-policy-runtime.json).

The mode/configuration restore restriction #1371 is also merged, but no
post-change Ubuntu seed/consumer measurement is claimed. PR #1374 merged and its master Qt producer (job 117551485401, run
39084764880) passed: 298/300 cacheable hits, a new
`qt-ccache-linux-inputs-v1-7d748b6e…` snapshot, and a 5-second save step.
[Producer metadata](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-qt-identity-producer.json) records the
exact key and bytes. The subsequent docs-only consumer, job 117554246917 in
run 39086280608, passed its 15 registered Qt tests with the exact same key,
298/300 hits, and its Qt
snapshot save step skipped. It reused the 1,161,888,219-byte archive without
uploading another Qt snapshot. Build/tests still ran (74 s versus the
producer's 54 s); these unmatched runner observations do not establish an
end-to-end speedup. [Consumer and upload evidence](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-qt-identity-consumer.json).

The producer's parent workflow later failed the separate recording-wait
assertions. A proposed private-helper patch passes two controlled ordering
cases where the original fails, and ten invocations of the unchanged test
(40 cases). It was offered on the existing #1283 claim for review;
production source and assertions remain unchanged, and real app shutdown
validation is still required. [Patch, probe source and results](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-recording-wait-proposed-patch.json).

The unchanged group-hide audio test passed three isolated and sixteen
concurrent local invocations on four pinned CPUs (76 cases, no skips). This
negative sample did not reproduce #838; serial execution preceded concurrent
execution without cold/warm Mesa cache control, and local binaries were not
matched to the GitHub revision. No scheduling or assertion change was made.
[Local probe and limitations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-audio-resource-probe.json).

### Subsequent consumers and availability failures

[Consumer and capacity observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-cache-consumer-followup.json)
record actual reuse after the host/container namespace split (#1354):

| Consumer | Cook result | Observed timing / outcome |
| --- | --- | --- |
| #1360 queue Fast Gate, documentation cohort | Exact host dev hit; 16,213 restored files | Setup 24 s, lint 53 s, job 97 s; successful |
| #1362 initial PR Tier 2 build | Exact container release hit; 542 restored files | Setup 12 s; later standalone compiler-cache daemon failure |

Both new namespaces have observed master producers and later consumers.
These runs do not establish a product-code speedup or resolve the Rust
stale-source blocker. A cook archive hit and a standalone compiler-cache
daemon failure describe different layers.

The latter failure preserved the first build's lost daemon connection and
subsequent connection-refused/cooldown messages. #1362 adds original-log and
daemon diagnostics, plus uncached real compilation during the existing single
recovery attempt when daemon status confirms unavailability. Local testing
with checksum-verified zccache 1.15.5 confirms unavailable status exits 1,
valid C compiles with `ZCCACHE_DISABLE=1`, and deliberate invalid C still
fails with its current diagnostic. Latest PR run
[39081664635](https://github.com/obs-rust/obs-studio/actions/runs/39081664635)
passed the build but failed the unchanged `test_recording_shutdown_wait`
state assertions in both modes: each registered 295 tests with one failing
test executable. The startup diagnostic passed in both. Source inspection
found a race candidate: the wait may return on `deactivate` before the output
reports inactive. Evidence is posted under the existing claim in
[#1283](https://github.com/obs-rust/obs-studio/issues/1283#issuecomment-6243649634);
no rerun, skip, or assertion change was made. This is not evidence of a cache
recovery failure. The daemon-death cause remains unknown, and #1362 is not
yet merged. [Raw validation summary](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-recovery-pr-validation.json)
keeps build success separate from test failures.

The full dispatch's native Windows Tier 2 compiler cache hit 308/308
cacheable calls, with 27 noncacheable calls. macOS Tier 2 passed 163
registered tests per mode in 112.59/100.78 seconds. The accompanying eight
soldr consumer observations preserve literal cook and embedded compiler
summaries separately: zero embedded session hits do not negate the Windows
C/C++ cache's independently observed hits.

Capacity pressure is now quantified without claiming a deletion actor.
The starting inventory contained 8,380,452,969 bytes. Known new archives
written before the 22:33 Tier 2 miss, and still present in the later
inventory, added at least 3,777,435,796 bytes. Retaining both sets would
require 12,157,888,765 bytes, exceeding the 10 GiB budget by 1,420,470,525
bytes. This is a lower bound on required removals, not an observed peak.
The oldest accessed survivor was newer than the lost Tier 2 seed, and no
known janitor ran during the relevant interval. Global LRU eviction is a
supported inference; manual/external deletion remains unexcluded.

The impact reached the queue: #1353's subsequent merge-group build missed
the compiler archive and compiled all 783 units cold (290-second build step,
395-second job). Source and runner differences prevent a matched timing
attribution. Family isolation alone has not established durable retention.

That product-code queue run completed successfully in 843 seconds from run
creation to required-gate completion. Its Fast Gate job (209 seconds) ran
entirely within the Tier 2 build job (395 seconds), verifying 209 seconds
of actual overlap on a code change. OFF/ON each registered 293 tests with
zero failures; CTest times were 316.03/316.71 seconds. Their concurrent job
phase took 390 seconds and consumed 772 runner-seconds. All non-skipped
jobs together consumed 1,499 runner-seconds; these are observed job durations,
not a monetary billing calculation. The older code-queue baseline median
was 954 seconds across eight samples, with different source, test counts
and runner conditions. One new sample does not establish a causal speedup
or replace the required representative-day comparison.
[Raw critical-path observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-code-queue-critical-path.json)
include the independent OFF/ON results and coverage caveats.

### Native Rust profile verification

Master [39063249804](https://github.com/obs-rust/obs-studio/actions/runs/39063249804)
passed native Rust test jobs on both platforms with `--profile dev`:

| Platform | Setup | Cook inside setup | Rust tests | Post-job setup-soldr save |
| --- | --- | --- | --- | --- |
| macOS arm64 | 34 s | 23.1 s | 89 s | 13 s |
| Windows x64 | 53 s | 30.0 s | 125 s | 15 s |

[Raw job observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-native-rust-cook-rollout.json)
verify profile use and successful consumers. Compiler-wrapper session counters
were zero in both jobs; these are separate from dependency-cook reuse. The
parent master run was later canceled after its native jobs and cache writers
finished; this is not a full-workflow pass or a matched speed comparison.

Windows native Tier 2 also passed in this master run, using `--release` for
the static-library consumer and recording **304/305 cacheable ccache hits**,
one miss. It saved `windows-tier2-ccache-6d7b7f7a...`. These C/C++ counters
are independent of the zero embedded zccache session counters printed by
setup-soldr. [Job timings and counters](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-windows-tier2-cache-rollout.json)
confirm that useful native compiler reuse survived the changes.

macOS native Tier 2 passed both implementation runs too: 161 registered,
160 passed and one `test_svt_av1` skip each, with CTest times 96.45/101.15 s.
Its release-profile consumer succeeded and saved `macos-tier2-xcas-6d7b7f7a...`.
The embedded compiler session recorded 22 hits and zero misses; this does not
establish an Xcode compilation-cache hit rate.
[Native macOS observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-macos-tier2-cache-rollout.json)
keep these independent scopes explicit. The obsolete master workflow was
canceled after its native jobs and cache writers completed; it is not claimed
as a full-workflow success. Full nightly verification remains separate.

### What still dominates test time

The [serial-test profile](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-tier2-serial-profile.json)
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
remains disabled. [Per-language results](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-nightly-codeql-verification.json)
record durations and job IDs. This proves nightly/manual scanning works; the
full nightly run subsequently finished with the macOS integrity failure described below.

Trusted scoped-plugin warming is now observed: master run
[39064132902](https://github.com/obs-rust/obs-studio/actions/runs/39064132902)
saved `targets-ccache-linux-713ceb3b...`, a 25,761,103-byte archive on master.
The touched-target build and save steps succeeded. The prepared measurement
PR selects only `linux-v4l2`; its tracked compiler-input fingerprint matches
the seed revision, excluding Markdown and generated version metadata.
[Experiment records](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-plugin-cache-experiment.json)
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

[Final job results](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-nightly-final-results.json) show
successful Ubuntu OFF/ON app builds, macOS Intel/Apple Silicon, Windows
x64/arm64, Flatpak, security analysis, Linux OFF/ON Tier 2 and Windows Tier 2.
The **overall run failed** at macOS `test_ingest_refresh_wait` /
`failed_refresh_ends_wait`: its elapsed time exceeded 150 ms. The first
implementation run failed, so the second implementation was not validated.
[Failure evidence](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-nightly-macos-failure.json) and
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

[Baseline log observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-16_17-ci-baseline-log-observations.json)
preserve the observed compiler counters, archive sizes, saved keys and test
counts. Logs were unavailable for 33 of 117 sampled attempts, all canceled;
missing counters are unknown. Repeated archive messages and independent
compiler scopes must not be summed into fictitious transfer/hit totals.

The [per-lane baseline](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-16_17-ci-baseline-lane-timings.json)
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

A [post-nightly inventory snapshot](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-post-nightly-cache-inventory.json)
records both cache APIs with collection timestamps. The itemized listing
contained 41 entries / 8,951,159,430 bytes; the usage endpoint reported 49 /
10,617,481,983 bytes. The pinned contract interprets `10GB` as 10 GiB
(10,737,418,240 bytes). These endpoints were asynchronous during concurrent
writers and janitor cleanup; their totals are not combined, and neither is
presented as steady-state headroom. An earlier listing peaked near 10.70 GB.
Nightly app archives dominate the transient footprint; useful queue caches
must remain warm through the full representative-day comparison.

### Confirmed retention pressure after rollout

[Capacity and eviction evidence](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-cache-retention-capacity.json)
records a confirmed janitor eviction at 20:12 UTC: the 711,970,692-byte Ubuntu
ON archive exceeded the shared `zccache` family's 1,782,579,200-byte allowance.
The same sweep removed the 1,083,740,873-byte Windows x64 app archive under
the shared Windows family's 838,860,800-byte allowance. These deletions are
distinct from the removed Ubuntu pre-build purge. The Ubuntu OFF archive's
disappearance remains unattributed.

The analyzed family declarations at `92605bf02` sum to 16,951,069,900 bytes against the
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

The queue-priority retention fix separates `zccache-tier2-linux-*` before the
broad `zccache-*` prefix. It reserves a 550 MiB Tier 2 family and leaves the
nightly families pooled at 1,100 MiB: combined 1,650 MiB versus the previous
1,700 MiB. Cache keys stay unchanged. An offline probe of the pinned janitor
reproduced deletion of a 475,427,223-byte Tier 2 seed after two newer Ubuntu
writes; with the split it protects Tier 2 and evicts the older nightly entry.
[Raw before/after results](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-tier2-cache-retention.json).
This isolates janitor protection without expanding its allowance. It does not
prevent GitHub's global-capacity eviction, retain both Ubuntu variants, or
resolve the overall declared-capacity mismatch. Live retention remains to verify.

A transitional sample from 20:38–21:45 UTC listed 32 runs and inspected 12.
The collector previously treated JSON evidence reports as product code when
they accompanied CI changes. The corrected classifier treats only JSON under
`docs/ci-evidence/` as documentation; product JSON remains code. Reclassifying
the saved actual diffs leaves the full-day baseline unchanged and moves four
transitional runs into CI/config. Its two successful queue samples (167.5 s
median) are CI/config, leaving **zero successful product-code queue samples**.
There is only one successful product-code PR sample (772 s). These sparse,
unmatched observations establish no queue speedup. See the
[cohort correction and preserved old/new counts](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-ci-cohort-correction.json).

The final after window must cover at least a full day of representative code
PR and queue traffic following rollout. It has not elapsed. The first host
cold/warm pair is complete. Repeatability over real traffic, Ubuntu app reuse
after the purge fix, the unresolved startup failure (#1355), stable cache
headroom and the representative-day comparison remain acceptance work under
#1286. This report does not claim those requirements are complete.

## Recommendation and acceptance checkpoint

### Rebuilt Tier 2 cache: observed warm consumer

After the archive disappearance, master `635454ee7` rebuilt 783 units cold
and saved a 188,194,495-byte compiler archive. Master `3ac86f74c` subsequently
restored it, recorded 734 cached / 49 cold compilations, and saved an updated
193,854,787-byte archive. Only documentation changed between these revisions;
generated revision metadata and runner placement still differ.

| Observed job | Complete build step | Whole job | Compiler hits |
| --- | ---: | ---: | ---: |
| Cold producer 117527420015 | 300 s | 383 s | 0/783 |
| Warm consumer 117530045228 | 117 s | 238 s | 734/783 |

[Raw producer/consumer observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-tier2-reseed-cold-warm.json)
establish useful reuse of the smaller new archive. These are single
observations, not a matched causal benchmark. Survival through another full
nightly write burst and representative-day traffic remains unverified.

### Expanded soldr lane observations

The earlier full dispatch also supplies
[eight soldr lane observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-full-dispatch-soldr-lanes.json),
including separate setup/post-step times and literal cook/embedded compiler
summaries. These are eight expanded jobs from the configured callsites,
not eight matched cold/warm experiments. Native test and static-library
consumers are kept separate from host cross-lint and Fast Gate. The Windows
Tier 2 standalone ccache hit 308/308 cacheable calls while its embedded soldr
counter was zero; those values describe different cache layers. Do not
disable the working native compiler cache based on the embedded counter.

### Existing artifact transport cost

The successful #1353 queue run uploaded its 177,835,306-byte execution tree
in 3 seconds. OFF/ON consumers downloaded it in 2/4 seconds and unpacked it
in 2 seconds each. [Literal step observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-tier2-artifact-transfer.json)
show inexpensive same-run transport for this payload. Cross-run producer
selection/authentication and deep Cargo/object-state payloads are separate,
unmeasured costs; this does not implement queue-only cache publication.

### Startup blocker: local failure identified

The failure diagnostics in proposed #1365 let the real unchanged slideshow
test identify `XOpenDisplay` failure during hotkey initialization under
concurrent startup/shutdown. On a shared default Xvfb server, six of 68
attempted invocations failed (workers stop at their first failure). With
only `-noreset` added, all 160 invocations and 1,920 startup groups passed.
The earlier Xlib-only probe had not reproduced this behavior. #1365 proposes
the server-lifetime fix for normal and repeated runs, preserving assertions,
parallelism and failure propagation. Full fresh OFF/ON CI and queue validation
remain pending; this local reproduction does not assign every historical
startup failure to the same cause.

### Remaining acceptance work

The follow-up fixes also incurred iteration cost. Across eight completed
runs on only the #1362/#1365 branches, canceled runs consumed 44.07 observed
runner-minutes and failed runs 58.08. Two active corrected runs are excluded
from this snapshot. [Per-run observations](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-followup-iteration-cost.json)
keep this separate from steady-state gains, billing and total #1286 rollout
cost. Failures included cache-daemon availability, startup and the new test
interposer; superseded runs also consumed work before cancellation completed.
These costs must not be hidden by reporting only successful final runs.

The pinned janitor's global-budget behavior is now checked directly:
`cache/ops.py::janitor` plans family/stale/finding cleanup and applies
newest-entry/grace protection, without a global trim against `[cache].budget`.
A [read-only synthetic executor probe](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-janitor-global-budget-probe.json)
kept two current 6 GiB entries when their individual limits were 6 GiB,
despite a 10 GiB global budget. That fixture intentionally has invalid
aggregate declarations; it is not a live inventory or a claim that static
budget checks accept it. Lowering the global budget field alone would not
create retention headroom. Whole-budget allocation and writer-burst policy
remain tracked in #1269; family isolation is insufficient by itself.

Keep the merged queue policy, Rust correctness safeguard, bounded useful
compiler caches and parallel OFF/ON runners. Validate the observed host-package
benefit over real traffic; an archive HIT alone is not a throughput result.
Validate Ubuntu app reuse after removing the pre-build purge, and compare a
full day of code PR/queue traffic after the final cache rollout. The
macOS integrity failure was resolved by #1338. The startup fix in #1365
has passed PR, queue, master and full native validation; #1355 is closed.

| #1286 requirement | Evidence/status at this checkpoint |
| --- | --- |
| 1. Match cook profiles and measure consumers | Explicit profiles landed in #1318; local dev/release/no-cook experiment and native consumers recorded. Representative post-rollout comparison remains pending. |
| 2. Rust cache correctness | Reproduction still fails on pinned soldr; safeguard retained in #1290, upstream zccache#1943 linked. |
| 3. Remove unused overhead | #1372 disables unused embedded transport in all seven callsites; #1380 reclaimed 183 MB of retired archives. Qt exact-key reuse skips duplicate publication. #1371 mode-compatible Ubuntu producers and later OFF reuse passed; remaining misses and whole-budget retention remain unresolved. |
| 4. Warm scoped plugin builds | PR consumer verified 41/41 compiler hits, 323 packages and zero package download bytes; first unmatched job pair 165 → 148 s. Both seed families were later absent. Warming works; durable retention and day-scale benefit remain unresolved under #1269. |
| 5. Persist test timing | Nonempty OFF/ON producer, save, later restore and CTest consumption verified across master runs (#1288/#1311). |
| 6. Match remote save policy | All seven setup-soldr consumers use master-only remote saves; PR/queue/local-save behavior verified (#1291). |
| 7. Bound Qt cache | 1.2 GB compiler bound, 1.3 GB family, actual high hit rates and restore/save times verified (#1309). Overall repository headroom still needs the full-day view. |
| 8. Unify nightly cleanup | Competing purge removed and startup permission failure fixed (#1289). #1338 resolved the macOS assertion; #1365 startup correction passed queue/master/full validation. Latest full run 39089156757 passed all seven app builds and nightly checks. |
| 9. Isolated libobs measurements | Three trials per condition and raw wall/CPU/cache evidence published in #1306. Wall-time ranges overlap. |
| 10. Shorten critical path | Independent OFF/ON runners and Fast Gate/build overlap landed. Product-code queue observation: required gate 843 s; Fast Gate 209 s entirely overlapped with build. Runner cost and coverage recorded; causal/day-scale gain remains unproven. |
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

[Raw trials, compiler settings, environment and cache statistics](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/libobs-2026-10-17/)
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
benchmark. [Captured result](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-rust-cache-source-probe.json).
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

[Summary, outcome counts, listing coverage and per-lane medians](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-16_17-ci-baseline-summary.json)
and [sample IDs, diffs, gate and runner times](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-16_17-ci-baseline-samples.json)
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
use it. [Captured sample coverage](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-gate-cache-consumption.json).
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

There is also a GitHub-native alternative to master-side promotion: queue
jobs can publish compressed build-state artifacts, and consumers can select
and download them across workflow runs. The official
[artifact action](https://github.com/actions/download-artifact#download-artifacts-from-other-workflow-runs-or-repositories)
supports this with a run ID and token possessing `actions:read`. This means
literal queue-only production does not necessarily require external storage.
Artifact storage is separate from the native cache allowance, with different
[storage accounting](https://docs.github.com/en/billing/concepts/product-billing/github-actions).

A read-only feasibility check listed #1353's successful queue artifacts and
downloaded its tiny lane-proof marker from a separate CLI session. Its
177,835,306-byte Tier 2 execution-tree artifact remained available with one-day
retention. This verifies discovery and authenticated cross-run access, not
large-payload transfer performance or fork-job token behavior. The execution
tree lacks objects/Cargo state; it is not itself a deep incremental cache.

Before choosing this design, measure one queue-produced build-state archive
and later PR consumption; validate producer repository, merge-group event,
successful required gate and compatibility metadata; bound retained generations
and verify fork access. Preserve executable permissions inside the archive,
keep miss/expiry fallback to normal compilation, and retain current-source
invalidation safeguards. No artifact-backed cache producer or writer-policy
change has been implemented.

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

[A manual deep-cache probe](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-deep-test-cache-probe.json)
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

### Native fingerprint helper evaluation

Standalone zccache 1.15.5 already exposes `snapshot-fp-record` and
`snapshot-fp-validate`, so the audit tested those native APIs before adding
production package-mapping logic. Both experiments kept the Rust wrapper
bypassed and preserved the modified nested source's size and nanosecond mtime
through hydration.

| Root package | Native source coverage | Validation report | Unit-test result after validation |
| --- | --- | --- | --- |
| `obs-deep-cache-probe` | 0 crates / 0 sources | 0 clean / 0 dirty | Old binary passed; current compile error absent |
| `obs_deep_cache_probe` | 1 crate / 2 root sources; hyphenated dependency missing | 0 clean / 1 dirty | Old binary passed; current compile error absent |

The naming failure is concrete: the recorder looks for a `.d` file using the
hyphenated fingerprint-directory basename, while Cargo emits a normalized
target filename with underscores. The same construction remains in the
inspected upstream source. It is tracked in
[zccache#1959](https://github.com/zackees/zccache/issues/1959).

The control also establishes a separate semantic limit. Native validation
recognizes changed source when covered, but only future-stamps clean units;
it leaves dirty Cargo metadata intact. It therefore cannot alone enforce
freshness for the preserved-timestamp case. The existing known-package
quarantine rejects the edit and retains the dependency in both fixtures;
automatic production mapping and generated-input coverage remain unfinished.
[Versions, source coverage, CLI outcomes and controls](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-18-native-fingerprint-helper.json).

Reproduce with a new directory and the pinned executables:

```sh
uv run ci/deep_test_cache_probe.py \
  --directory build_ci_audit/native-fingerprint-new \
  --soldr /absolute/path/to/soldr \
  --native-fingerprint-tool /absolute/path/to/zccache \
  --verify-native-guard
```

The current pins return 1 because native coverage/invalidation is insufficient.
Repeat in another new directory with `--package-name obs_deep_cache_probe`
for the naming control. Existing `--verify-guard` still verifies only the
known-package quarantine. These are manual experiments; no native helper
was enabled in production CI and no tests were skipped.

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
claimed. [Raw results](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-obs-util-build-inputs.json).

The upstream replay-precision issue remains
[soldr#3851](https://github.com/zackees/soldr/issues/3851). This mitigation
does not address same-stat source edits or justify removing the correctness
safeguards. Native Windows input tracking still requires CI validation.

### Host and container cache identities

The pinned janitor's superseded-generation check groups by family and key
shape, ignoring GitHub cache version. An offline probe marks a valid host
archive superseded by a newer container archive with the same cook key and
different version, even though both fit the family allowance. The observed
18,020,243-byte Linux release entry's version matches the container archive
path, not the host path. This does not prove a particular historical deletion.

Use the existing `cache-key-suffix` input to distinguish `linux-host-v1`
and `linux-container-v1`, keeping compatible host jobs together. The pinned
superseded-generation check retains both namespaced fixtures. Profiles and
master-only saves stay unchanged. The input also namespaces the action's Rust
build-cache: first consumers can miss until trusted master seeds exist.
Registry, toolchain, mini and standalone C/C++ keys stay shared as before;
disabled layers stay disabled. No warm entry is forcibly deleted.
[Identity probe and side effects](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-cook-layout-namespaces.json).

Full validation's Fast Gate independently reported a base restore miss and
later reservation conflict. Its cause remains unproven; namespace isolation
is not evidence that this conflict is fixed. Measure fresh seeds and later
consumers before claiming improved deep reuse or a throughput gain.

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

[Raw trial results](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-17-soldr-cook-post-job.json) include
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

# Flatpak performance retrospective (#1760)

## Measured performance result

| Metric | Historical baseline | Final configuration |
| --- | ---: | ---: |
| Warm whole-job samples | 702s, 905s, 911s | 437s, 434s, 447s |
| Median | 905s (15m05s) | **437s (7m17s), 51.71% lower** |
| Nearest-rank sample p95 | 911s | 447s |
| Sample size | 3 | 3 |
| Matched current cold | — | 846s |

The final three warm full-job observations are **437s, 434s and447s**.
Their median is **437s (7m17s),51.71% lower** than the historical905s
(15m05s) baseline median. This meets the >=50% and <=450s performance target.
Nearest-rank sample p95 is447s versus911s before; with n=3 these are sample
maxima, not stable population-tail estimates. Matched cold is846s.
All selected final workflows failed independently on #1816; successful Flatpak
jobs do not establish healthy whole-workflow CI. At this report revision, final retention, reporting,
review/merge and owned cleanup remain. The closing report on #1760 records
those terminal verification results after normal queue maintenance.

Implementation PRs #1763 (CEF), #1771 (installed saved-profile validation),
and #1767 (safe compiler reuse and projected cache) merged through the normal
queue. #1803 adds default-off compiler attribution and has merged normally.
#1806 restores browser C++20 cacheability and merged normally after SDK, full
consumer and installed saved-profile qualification passed.
No new workflow files were introduced. The original three-sample cohort and
matched cold measurement are complete, as is the final C++20 production cohort.
The issue remains open for settled retention, reports, review/merge and owned cleanup.

The completed table below describes the intermediate configuration before #1806.
The final-configuration result is reported above and below.

| Metric | Before | Interim measurement (before #1806) |
| --- | --- | --- |
| Warm full-job observations |702s,905s,911s|480s,421s,475s |
| Median |905s (15m05s)|475s (7m55s),47.51% lower; misses target |
| Nearest-rank sample p95 |911s (n=3 maximum)|480s (n=3 maximum), not a stable population tail |
| Warm compiler result |Old reports were restored cumulative counters|Cacheable calls:329/331,327/330,328/330 direct hits after reset;16unsupported calls excluded in each |
| Actual projected archive |Legacy builder2,715,273,348B plus59,423,648B compiler in production seed|2,266,246,245B seed;2,266,212,985B next generation |
| Current cold whole job |Historical explicit cold builder790s;2111s diagnostic whole job included extra probes|Matched090c8ad cold-only39820593708:921s,SDKpreinstalled |

First warm run39812201578/job119942032116 genuinely rebuilt OBS, passed all
validators in480s and its complete selected workflow passed with112.75
runner-minutes. Second run39816841905/job119958260969 genuinely rebuilt and
published a new source generation in421s; its complete workflow passed,118.65runner-minutes.
Third run39821239168/job119973699055 passed475s and115runner-minutes.
All three selected workflows passed without failure/cancellation,346.4runner-minutes
total. Matched cold39820593708 passed921s/120.55selectedrunner-minutes,
with0compilerhits/330misses/16unsupported; SDKimage inputs remain preinstalled.
Its source090c8ad includes the portal shortcut-set port #1800 relative to
2bc7b591 in the first sample. Manifest/browser/CEF profile is unchanged;
compile call counts and source differences remain disclosed. These are
observed before/after cohorts, not a matched-source causal estimate.

## Final configuration seed and measurement routing

Trusted manual master seed [39831165145](https://github.com/obs-rust/obs-studio/actions/runs/39831165145)
used source `e1b649910c0d082b482bd3adfe58c67436b3e82a`.
The full Flatpak job passed in **455s**, with six dependency hits, a genuine
application rebuild, 315 direct compiler hits, 30 misses and zero unsupported
calls. It published archive `9358763488`, **2,274,967,809 bytes**, after the
required validators. This producer seeds newly cacheable browser objects and
is excluded from the final warm cohort.

The complete workflow failed in Fast Gate on the shader sampler C oracle's
strict sign-conversion compilation ([#1816](https://github.com/obs-rust/obs-studio/issues/1816)).
All selected jobs consumed **81.47 runner-minutes**; the failure is retained,
with no unchanged retry. A successful Flatpak job does not imply a successful
whole workflow.

Actual merged path scope did not select a full Flatpak job on master push;
the manual seed corrects the original automatic-seed assumption. Before any
final warm sample, the [amended sampling plan](https://github.com/obs-rust/obs-studio/issues/1760#issuecomment-6348807912)
proposes the default-off manual routing mode in #1817. That mode keeps the
complete Flatpak job and Fast Gate while omitting unrelated native matrices.
Its branch qualification passed the complete Flatpak job in392s, with342direct
hits/3misses, all six dependencies reused and all three artifact validators
passing. Fast Gate reproduced #1816 and the final gate correctly failed;
whole workflow cost14.033runner-minutes, no cancellations or retries.
[Portable routing evidence](ci-evidence/2026-10-22-flatpak-measurement-routing.json)
records the source and skipped native matrices. The branch retains existing
trusted-master publication restrictions and is excluded from the production cohort. Routing
savings cannot count toward the Flatpak job's performance target.

## Final production cohort

Normal routing PR #1817 merged as `9b6f1f74835cfb852d70dd10abfb0c070058d8d5`.
The [first final warm run](https://github.com/obs-rust/obs-studio/actions/runs/39837821816)
passed its complete Flatpak job in **437s (7m17s)**, including required new
cache publication. All six dependencies were reused, OBS genuinely rebuilt,
and the compiler reported342directhits/3misses. All three artifact validators
passed. Restored archive9358763488 was2,274,967,809B; new archive9360891151
is2,274,822,651B. The complete workflow failed on known #1816 and consumed
14.967runner-minutes. No cancellation or retry was used.

Warm2 passed434s with343directhits/2misses, all six dependency hits and all
three artifact validators. It restored archive9360891151 and skipped immutable
exact-key publication. Its whole workflow failed on #1816,17.45runner-minutes.
Exact-source cold passed846s with0hits/345misses, all artifact validators,
and all four builder/compiler restore steps skipped. Publication was disabled;
the SDK image remained preinstalled. The cold workflow failed on #1816 and
consumed20.567runner-minutes. Third warm39840480759 passed447s,331directhits/14misses,
all six dependency hits and all validators, including new-generation publication.
It includes #1815 scene-audio extraction relative to the first two samples;
recipe/browser/CEF are unchanged. Its workflow failed #1816,17.017runner-minutes.
The final median437s meets the target; selected warm workflows cost49.433runner-minutes
with three failures, no cancellations or unchanged retries.
[Portable cohort evidence](ci-evidence/2026-10-22-flatpak-final-cohort.json)
retains all three observations and their whole-workflow outcomes.

## Where the time went

Observed nonoverlapping log intervals for reference39721238636/job119635024523
and production warm 2, run39816841905/job119958260969:

| Phase | Reference: 911s | Warm 2: 421s |
| --- | ---: | ---: |
| Setup/restores/dependencies through OBS module start |207s|174.34s |
| OBS checkout/configure/compile/install |340s|116.97s |
| Debug processing through build-stage commit |210s|10.39s |
| Stage commits/cleanup/export through counter report |121s|77.89s |
| Stats/validators/publication/final cleanup |33s|41.41s |

For the final configuration, [three completed warm phase observations](ci-evidence/2026-10-22-flatpak-final-phases.json)
show167–192s setup/restores,81–89s configure/compile/install, about14s debug
processing,117–120s commits/export and28–50s counters/validators/publication/cleanup.
These sum to the complete437s/434s/447s jobs; setup and export dominate remaining
wall time. First stripping messages mark the observed debug phase.

These are buffered-log wall intervals, not CPU profiles. In the interim table above, the final phase
includes required new-generation publication on warm2; warm1 had an exact
key hit and skipped immutable-key publication. Do not add independent
fixture/probe percentage savings together.

## Changes adopted

- CEF keeps usable build IDs/debug source lines, with zstd-compressed ELF
  debug data and first-party no-debuginfo-compression. The isolated pinned
  CEF comparison was130.951s to10.666s; real export/runtime checks preserve
  browser loading, the unchanged C++ WebSocket v5 and required modules.
- Compiler caching uses real compiler paths, PCH-aware CMake launchers,
  dependency mode and compiler-content validation. Ten SDK trials cover
  preserved-size/mtime source/header changes, compiler/config changes and
  following warm hits. Rust compiler wrappers stay disabled; Rust targets
  are not retained in the projected archive and actual rebuild logs show
  the Rust crates compiling.
- One trusted master publisher saves only dependency OSTree and compiler
  state after validators and successful existing pruning. New-state hits
  suppress legacy overlays. The pinned action and standalone compiler
  writers are disabled. Existing backend, budgets and janitor policy stay.

## Techniques measured and deferred

| Technique | Evidence | Disposition |
| --- | --- | --- |
| Disable transient cache fsync |Upstream builder already disables per-file cache fsync|No change |
| Lower archive zlib level6 to1 |Full real payload, four ABBA trials:6=[25.08,50.41]s;1=[21.40,23.76]s;all five refs/17,914objects verified;49,173,191B larger|Defer: noisy local15.17s median difference, no demonstrated whole-job gain |
| Custom production transport |Actual combined experimental consumer/runtime succeeded, but existing Actions projection fits declared family|Existing backend preserved |

The first full-export experiment correctly rejected a screenshots ref mismatch:
the source summary referenced an older commit than the source loose ref.
Corrected experiment535 imports exact immutable commit IDs and retains all-ref
identity/fsck guards. It verifies the original artifact SHA256 before unpacking.
No packaging setting changed from this local experiment.

## Runtime and invariants

V8 consumer export App dcec8109... /Debug1efed457... matched the installed refs.
The saved screen/camera/microphone/browser profile passed actual recording,
310browser frames, tone/motion checks, authenticated unchanged C++WebSocket v5,
23module inventory, CEF build ID/GDB source line and clean shutdown. Synthetic
devices and software rendering do not establish physical-device/GPU coverage.
Existing C tests and public headers were not changed for these units. Default
release bundles and debug artifacts remain intact.

## Capacity and transfer accounting

Pinned ci-lint interprets GB as1024^3: global10,737,418,240B;
builder2,684,354,560B. Each real2.266GB archive fits the unchanged builder
allocation. Two retained production consumers prove useful reuse. The later complete snapshot9,186,986,336B contains one projected generation
and fits the existing global limit. Janitor119982543166 succeeded and reports
one builder entry; it deleted three superseded non-Flatpak entries. Ownership
remains with #1269; the earlier seed eviction actor is not established. Normal janitor LRU overflow/newest-entry
protection is preserved; no manual cache deletion or invented capacity.

Archive sizes are actual cache/artifact payload observations. They are not
complete NIC/TLS transfer totals. Excluding downloads means source archives
are fetched again: warm2 logs seven source downloads and1,026,740,380B apparent
download-directory inventory. That inventory is not wire bytes, and reduced
cache archive size alone is not a claim of lower total network traffic.

## Cost and failure accounting

All measurement workflow job intervals (all attempts) are retained in
cost-ledger/summary.json, with raw responses per run. The three baseline
selected workflows cost374.033runner-minutes; their Flatpak jobs alone
cost41.967runner-minutes. Completed nonbaseline workflows currently account
for2199.017runner-minutes, excluding a cancelled seed workflow whose
112.117known runner-minutes have one inconsistent cancelled placeholder. This is runner-interval
accounting, not an invoice. PR/merge-group CI and local bosn work are explicitly
outside this ledger scope.

The production seed Flatpak passed651s with0hits/331misses, but its remaining
macOS job was automatically cancelled when the next master dispatch took the
same workflow concurrency slot. The raw all-attempt API also contains one
unexpanded cancelled matrix placeholder with a reversed1s interval; report
this inconsistency rather than silently adding negative time. Later master
measurements wait for their entire predecessor to become terminal.

Failures are preserved: wrapper-link packing, incomplete container upload,
bootstrap PATH, SDK-profile mismatch, harness readiness/profile/recording
checks, and native media-seek/package-acquisition failures. V7 Qt/runtime
package setup timed out automatically. No unchanged workflow was retried to
obtain a passing result. Changed-code trials and their extra native/transport/
runtime costs remain separate from routine Flatpak improvements.

## Post-cohort cache observation

The [post-cohort inventory](ci-evidence/2026-10-22-flatpak-post-cohort-retention.json)
contains39entries/11,139,864,070B, above the10GiB global budget. Its two projected
archives total4,549,850,337B, above the2.5GiB family budget. This is a transitional
snapshot, not settled capacity proof. The newest archive9361847609 is2,275,027,686B
and fits individually; final warm2/3 actually consumed9360891151. No claim is made
that the newest archive has a later consumer. Existing janitor maintenance at
normal merge-group/master events must demonstrate settled retention; its terminal
log/inventory and final reporting are recorded in #1760's closing report. Backend,
budgets and age protection are unchanged; no cache was manually deleted.

## Handoff checkpoints at this report revision

1. Finish the final portable evidence review against raw records; all three
   final warm observations and exact-source cold are complete.
2. Preserve the historical and interim cohorts, source/toolchain differences
   and the distinction between Flatpak success and failed whole workflows.
3. Verify retained production consumer reuse and current family/global capacity;
   preserve all run costs, failures, cancellations, bytes and source differences.
4. Commit portable final evidence and publish final1760/1667reports, including
   the measured target result and remaining CI failure; audit all original criteria.
5. Finish only owned branch/worktree cleanup. The original checkout now holds
   another active release change; preserve that work and its branch.


## Current browser compiler follow-up

Corrected diagnostic39820372855 attributes all16unsupported calls to obs-browser
`-fmodules-ts`. Recipe-only CMAKE_CXX_SCAN_FOR_MODULES=OFF retains C++20 and
header dependency tracking. RealSDKRED545warm1hit/1unsupported fails; GREEN549
passes all10source/header/compiler/configuration freshness trials and warm2hits.
The original production cohort median475s misses the450s/50% target; preserve
allthree samples and measure the new configuration separately after qualification.

After normal1803queue merge, cache snapshot records36entries/9,186,986,336B,
below the binary10GB global limit; one projected generation9354045170 remains
at2,266,212,985B, below the binary2.5GB family limit. Do not infer which actor
evicted older generations solely from this inventory; inspect janitor logs.


## C++20 full qualification outcome

Producer119984644801 passed470s with327hits/19misses. Fresh consumer119988550402
passed558s with343hits/3misses/0unsupported, all6dependencies, genuine OBS rebuild,
fsck10commits/19,088objects and all validators. Dependency and compiler restore
helpers took18.905s and2.285s; these exclude download time. Actual input artifacts
were1,627,039,061B dependency and648,930,822B compiler; exported repository artifact
was1,095,997,602B. Installed saved-profile runtime119993180570 passed133s, with
303verifiedbrowserframes and exact consumer App/Debug identities, CEF debugger
and unchanged C++WebSocket checks. Synthetic/software limitations remain.

The enclosing qualification39824614077 cost133.3667runner-minutes and failed
only the known Windows callback parity test (664pass/1fail, #1549/#1541); its seed
and log were preserved without rerunning or changing tests. Normal1806PRCI and
merge-result CI passed independently. The PR has merged through the normal queue. The later production cohort establishes the final performance target, with
whole-workflow failures and source differences retained.

## Process improvements for the next CI batch

- Manual full-matrix experiments spend most runner time on unrelated native jobs.
  #1817 adopted a tested manual lane selector in the existing workflow, retaining
  the full Flatpak job and Fast Gate while preserving routine PR/release gates.
  These routing savings are separate from the measured Flatpak job speedup.
- Freeze qualification source identity and fetch completed artifacts alongside
  lagging status APIs. A timeout observing a run is not a build failure or retry.
- Record all compiler calls, including unsupported calls; reporting only cacheable
  hits hid16expensive browser compiles behind a99%hit figure.
- Inspect GitHub mergeability early when PR checks are absent. This unit had an
  adjacent documentation conflict after the diagnostic PR merged; resolving it
  preserved byte-identical recipe/fixture/test code and allowed normal PR checks.
- Keep one primary reviewer through follow-ups and one RED/GREEN unit per PR;
  preserve failed full-workflow costs separately from successful Flatpak scope.


Historical media-seek failures in v2/v3 were tracked by #1765/#1774 and fixed
independently in #1775 (b39a4e0a8). Those qualification branches predated the fix;
retain their actual failed outcomes/costs rather than attributing them to Flatpak
configuration. The later Windows callback-parity failure remains a separate
known issue. No C test was modified by the Flatpak performance units.


## Evidence index

- [Completed interim production cohort](ci-evidence/2026-10-22-flatpak-interim-cohort.json):
  all three observations, cold run, identities, costs and failed target.
- [C++20 SDK RED/GREEN](ci-evidence/2026-10-22-flatpak-cpp20-cache.json):
  all ten invalidation/reuse trials.
- [Full C++20 qualification](ci-evidence/2026-10-22-flatpak-cpp20-qualified.json):
  actual fresh consumer and installed App/Debug/profile identities.
- [Earlier transport qualification](ci-evidence/2026-10-22-flatpak-transfer-v8-qualified.json):
  original combined transport/runtime evidence and limits.

## Final cohort status

#1806 merged normally as e1b649910c0d082b482bd3adfe58c67436b3e82a.
The trusted manual master seed [39831165145](https://github.com/obs-rust/obs-studio/actions/runs/39831165145)
passed455s with all optional inputs off. It is excluded from the warm cohort.
Actual merged ci/ci_scope.py returns full=False for path-based master pushes,
so no automatic full Flatpak seed is selected; earlier scheduling assumptions
were corrected before observing any final warm timings. The first three routine
warm jobs and matched cold are now complete:437/434/447s and846s respectively.

- [Workflow cost ledger](ci-evidence/2026-10-22-flatpak-workflow-costs.json):
  all completed measurement runs, including final seed/cohort observations,
  and the cancelled seed metadata anomaly.

# Flatpak PCH cache qualification (#1760)

This candidate activates the existing CMake ccache launcher so the existing PCH
helper supplies `-fpch-preprocess` and its bounded PCH settings. Explicit SDK
compiler paths avoid nesting the SDK's `/run/ccache/bin` masquerading wrappers.
It does not enable a Rust compiler wrapper or change cache transport.

Enabling the launcher alone is unsafe: a real fixture changing `#define VALUE
42` to `43` returned **42**, including with direct mode disabled. Ccache correctly
rejected the direct manifest, then reused an old preprocessor result while
creating the PCH. The macro does not appear in that preprocessor output until
used in the consuming translation unit. Dependency mode avoids this fallback;
compiler content checking also detects compiler replacement with preserved
size and modification time.

The manual diagnostic exercises the manifest's actual ccache settings and
unchanged OBS PCH helper. Run inside the production SDK, with this checkout
available, and retain the new output directory:

```sh
uv run --no-project ci/flatpak_pch_probe.py --output /path/to/new-evidence-directory
```

It compiles and executes a tiny C++ program, then requires correct output after
same-size, same-timestamp header, source and compiler changes, and after a
configuration change. Each following clean build must reuse both cached objects.
An isolated cache prevents inherited counters from being mistaken for this run's
hits. It intentionally preserves logs, counters and outputs for investigation.
This fixture is manual and is not a full application build or a timing gate.

Local GCC/ccache evidence: ten trials passed. The header, configuration and
compiler changes each caused two misses; a source-only change retained one PCH
hit and missed the source object. All five warm builds had two direct hits.
Baseline wrapper-only warm rebuild: 1.219 seconds with two PCH refusals; the
initial dependency-mode launcher fixture: 0.242 seconds with two direct hits.
These are single tiny-fixture observations, **not a claimed full-job speedup**.

## Production SDK toolchain check

The same ten trials pass with the production SDK's **GCC 16.2.0 and ccache
4.14**, using its native binaries, libraries and headers in an isolated chroot.
See [portable SDK evidence](ci-evidence/2026-10-22-flatpak-pch-sdk-freshness.json).
Every following warm build has two direct hits; header, configuration and
compiler changes each miss twice; the source-only change misses once and retains
the PCH hit. The fixture's compiler wrapper changes actual executable output;
this is stronger than checking cache counters alone.

The local Flatpak namespace launch was denied by the container. The fallback
initially timed out copying the whole SDK, then exposed the SDK's empty passwd
file: ccache could not resolve its user identity. A separate root with a real
passwd entry, as Flatpak normally supplies, fixes that setup failure. These
failures are recorded; they are not compiler freshness failures or full-job
observations. The diagnostic now retains failing command arguments and stderr.

## Restore newer compiler metadata last

The builder archive contains `.flatpak-builder/ccache`, including mutable
ccache direct-mode manifests. Restoring a newer standalone compiler snapshot
first, then extracting an older builder snapshot, discards newer manifest
mappings even though the content-addressed result objects remain present.

The production SDK fixture reproduced this using actual GNU tar archives:

| Restore order after header 42 → 43 | Output | Direct hits / misses | Build |
| --- | ---: | ---: | ---: |
| New compiler snapshot only | 43 | 2 / 0 | 0.977 s |
| New compiler, then old builder | 43 | 0 / 2 | 4.191 s |
| Old builder, then new compiler | 43 | 2 / 0 | 0.899 s |

Each trial uses a new cache directory, a clean build and zeroed statistics.
The header replacement preserves size and modification time; dependency mode
keeps every output correct. The snapshots are captured after the existing
fixture's cold and header-warm stages. Extract each ordering with `tar -xf`,
then rebuild that same fixture with `CCACHE_DIR` pointing at the extracted
cache. See [portable counters and timings](ci-evidence/2026-10-22-flatpak-cache-overlay.json).
An independent file-overlay experiment agrees. This demonstrates lost reuse,
not stale executable output or a whole-job speedup.

The workflow now extracts the builder archive before the standalone compiler
snapshot and disables the builder action's second restore. `cache: true` still
passes `--ccache`; master-only publication and architecture-specific fallbacks
are preserved. The explicit builder restore's exact-hit output prevents an
unnecessary save to an existing immutable key, since the action's internal
restore no longer supplies that information. No cache family, storage backend,
capacity allowance or pruning policy changes.

The workflow resets only compiler statistics after both cache restores, before
building, so the reported hits/misses belong to that build. Cached results and
manifests remain intact. Existing historical reports with cumulative counters
cannot establish a per-build hit rate. The reset runs inside the SDK already
supplied by the pinned build image and its cost remains within whole-job timing.

## Adoption hold

Real application freshness/reuse inside Flatpak, full-job warm/cold
measurements and the packaged screen/camera/microphone/browser/debugger scenario
are still required. Keep the PR draft until these are proven. This change alone
does not complete #1760's 50% whole-job target.

## Manual cold builder/compiler measurement

The existing workflow accepts default-off `flatpak-cold-build=true` on an
explicit dispatch. It skips both current/legacy builder archives and the
standalone compiler restore, and suppresses both publications. A fresh hosted
runner therefore starts with no Flatpak builder/compiler state. The full app,
ccache enablement, counter reset, export and all validators still run. Ordinary
PR/push/release selection and caching are unaffected; no cache is deleted.

Use the same frozen source, manifest and container image for a corresponding
warm observation. Include the whole job and disclose every restore outcome.
This is cold **builder/compiler state**, not a claim that the internet, registry
or preinstalled SDK is cold: the pinned build image already contains the SDK,
and uv/tool input caches retain their existing policy. Other full-dispatch
lanes still run and their runner cost must be reported separately. The controlled hosted observation below has executed this control.

## Full application compiler reuse probe

The existing default-off `flatpak-pruned-build-probe` now also requests
`--compiler-reuse`. The first isolated rebuild still copies only dependency
OSTree state; original downloads, compiler results and transient state are not
retained. After that rebuild creates its own compiler results, the probe resets
only its isolated counters and changes a post-install marker to force another
application build with identical source, SDK and configuration. All dependency
modules must hit, while the application module must actually rebuild.

The second build must report at least 100 compiler hits and an 80% cacheable
hit fraction; a few CMake checks or a cached application module cannot pass.
Both output sets run the existing AppStream/build-directory/repository checks.
The source state stays read-only, and temporary state is removed on failure.
The report keeps cold/warm build phases and SDK counter data separate. These
extra manual rebuilds are diagnostic cost, not routine full-job savings or a
substitute for cross-runner transport/retention and invalidation evidence.
The hosted observation below has executed this compiler probe.


## Hosted full-application compiler reuse

[Run 39768771734 / job 119790889408](https://github.com/obs-rust/obs-studio/actions/runs/39768771734/job/119790889408)
uses qualification source `2b253bc0dace3c35478d3a46664edc49f0db023f`.
All three builder/compiler restore steps were skipped and the statistics reset
passed. The initial full builder action took **790 seconds**, with zero hits,
330 misses and 16 uncacheable calls; all output validators passed.

The isolated probe then copied dependency OSTree state only. All six dependency
modules reused their cached stage while the application actually rebuilt:

| Isolated application rebuild | Build seconds | Cacheable hits / misses |
| --- | ---: | ---: |
| No inherited compiler state | 728.798 | Not used as a warm observation |
| Compiler state generated by the preceding build | 326.785 | 328 / 2 |

Both output sets passed the existing validators. The second build uses ccache
4.14.1 and has no PCH refusal or preprocessor-cache fallback. Its 99.39% hit
fraction demonstrates substantial real application reuse. The 55.16% reduction
is for these paired **build phases**, excluding copy, fsck, SDK queries and
validators; it is not the issue's median whole-job target.

The resulting compiler cache occupies 625,812 KiB (640,831,488 bytes). This
is larger than the current 200 MB compiler-family allocation, but is not an
Actions archive measurement. Retention and global capacity must be qualified
with #1269 rather than silently raising the budget.
Canonical tar/zstd proxies were 3,247,992,418 bytes full, 2,225,845,035 without
downloads and 1,589,558,859 without downloads/compiler state. They do not prove
actual Actions transfer or authorize pruning. One warm/fresh source-fetch pair
was 0.048/10.065 seconds, not a representative performance cohort.

The overall workflow has a retained macOS C-reference seek failure, reported
on #1765. Fresh-runner installed-runtime job 119806132310 passed the exported app with
a saved screen/camera/microphone/browser profile and debugger checks. It uses
synthetic devices and software rendering, not physical devices or hardware GPU.
The whole diagnostic Flatpak job took 2,111 seconds, including both extra
rebuilds and capacity probes. Buffered probe subprocess output means CI line
timestamps cannot attribute internal phases; use its monotonic build timers.
Keep the
adoption hold: this proof does not establish a retained cross-runner compiler
consumer, comparable warm whole-job measurements or the 50% whole-job target.
Portable raw probe results and exact input/limit metadata are recorded in
[the full-app compiler evidence](ci-evidence/2026-10-22-flatpak-full-app-compiler.json).


## Explicit fresh-runner compiler transfer experiment

Default-off `flatpak-compiler-transfer-probe` packages the completed producer's
compiler cache as an explicit Actions artifact retained for one day. The cache
results are already compressed, so the tar and artifact avoid another compression
pass. It records actual archive bytes, digest, payload occupancy and pack time.
The experimental bound is 2 GiB payload and 100,000 ordinary files; it is not a
production-family allocation increase. The caller forwards this input only for
an explicit manual dispatch.

A separate fresh-runner job restores dependency state first, then downloads and
overlays the compiler artifact. Before changing its cache, it checks the complete
archive digest/inventory, rejects links and unsafe paths, and requires an exact
source SHA, manifest digest, SDK commit, compiler hashes/cache version and
architecture match. It resets statistics, builds the full application, requires
at least 100 hits and 80% cacheable reuse, and runs all three output validators.
The builder action cannot restore over the artifact or publish either cache.
Consumer restoration/counters and producer metadata are retained separately.

This uses a visible same-run Actions artifact, not a new production transport or
cache family. Report artifact storage/transfer, both full job durations and all
selected workflow costs. It does not establish production retention, changed-source
freshness or representative medians. No hosted run has yet executed this route;
the adoption hold remains. The independently qualified runtime scenario and SDK
freshness trials remain required evidence alongside it.


### First transfer trial: builder passed, wrapper scaffolding rejected

[Run 39776059128 / producer 119815324383](https://github.com/obs-rust/obs-studio/actions/runs/39776059128/job/119815324383)
built and validated the full application, then failed during artifact packing.
No compiler artifact was published and neither fresh consumer nor runtime
qualification ran. The failure and all selected job costs remain part of the
experiment; this is not cross-runner reuse proof.

The source cache includes `bin/cc`, `bin/c++`, `bin/gcc` and `bin/g++` symlinks to
`/usr/bin/ccache`, created by
[flatpak-builder 1.4.9](https://github.com/flatpak/flatpak-builder/blob/1.4.9/src/builder-context.c)
in `builder_context_set_enable_ccache`. Its hosted inventory reports 60 bytes
under `ccache/bin`, matching those four launcher links. The ordinary-file-only
packer rejected this generated scaffolding. The initial fixture omitted these
links; the new RED regression reproduces the real source and destination layout.

The corrected experiment omits only these exact wrapper paths with the exact
upstream target, records their omission, and preserves matching destination
wrappers. It rejects every archive entry under `bin`, all archive links, and
unexpected source/destination links. This prevents a regular archive entry from
being copied through an existing compiler wrapper. Profile/digest checks,
substantial-hit requirements and production adoption holds remain intact.
A changed-source trial is required; no unchanged rerun was requested.


### Second transfer trial: packed cache omitted from upload

[Run 39779603900](https://github.com/obs-rust/obs-studio/actions/runs/39779603900)
uses source `030daf8b0e2e218109a55eb32e5fcff7b3bf3a16`. The producer built and
validated the app, and the wrapper fix packed 1,300 ordinary files into a
699,105,280-byte tar (698,094,555 payload bytes; 2.014 seconds). However, the
published artifact was only 1,488 bytes: it contained metadata at
`__w/_temp/flatpak-compiler-transfer/manifest.json` and no compiler tar. The
upload log reports a least common root of `/` and one file. Packing success
therefore did not establish transport success.

The fresh consumer separately failed before artifact download: its combined
bootstrap/vendor step could not find `soldr`. The bootstrap writes `GITHUB_PATH`,
which Actions applies to subsequent steps. The established producer already
separates those commands. The installed saved-profile runtime job passed, but
neither cache download nor cross-runner compiler reuse was proven.

The next revision uploads a single controlled producer directory. Consumer JSON
reports are staged into a separate single directory, avoiding another upload of
the large compiler archive. Bootstrap and vendoring are separate steps. Focused
RED regressions reproduce the missing upload/step boundaries; the corrected
configuration still requires a changed-source hosted trial. Production transport,
budgets and adoption holds remain unchanged. See
[the second transfer evidence](ci-evidence/2026-10-22-flatpak-transfer-v5.json).


### Cross-runner trial v6: download passes, profile guard rejects

Run [39784005854](https://github.com/obs-rust/obs-studio/actions/runs/39784005854)
at `c907b9ef1` is terminal failure. Producer119842391321 passed all validators
and uploaded two files: the actual compiler artifact is698,963,464 bytes
(tar698,961,920 bytes;697,941,503 payload bytes;1,300 files;2.378s packing).
Whole producer975s includes optional transport/runtime export; both builder
restores missed and isolated compiler counters were0hits/330misses. This is
not a warm performance sample.

Consumer119850594734 downloaded the artifact successfully and matched its
published digest, then rejected the profile before overlay or application
build. Both jobs pulled image digest`6ed6921bb5a9e1a916989971844f159b854b7b225880c381fa34f7a158863039`.
The producer builder explicitly updated SDK/runtime before compiling, whereas
consumer profile validation ran before equivalent dependency preparation.
The correction uses bounded dependency-only builder preparation before
profile validation; remaining drift still fails. A mismatch report now retains
both identities and differing fields. Focused RED/GREEN checks cover the
preparation order and rejection before destination mutation.

Installed saved-profile runtime119850594274 passed separately. The full trial
cost135.9333 runner-minutes across22successful and2failed active jobs, with
no cancellations; skipped jobs excluded and no billing multipliers. See
[portable evidence](ci-evidence/2026-10-22-flatpak-transfer-v6.json).
Actual compiler reuse, retained capacity, and routine whole-job savings remain
unproven. The runtime fixture uses synthetic devices/software rendering.


### Optional actual OSTree cache transport qualification

`flatpak-dependency-transfer-probe` defaults to false and requires the compiler
transfer probe. It adds an actual tar/zstd archive of `.flatpak-builder/cache`
to the same-run experiment, with one-day artifact retention. This is a cache-only
projection; application stage objects can remain. It is not a minimal dependency
graph or a production writer/allocation change.

The helper accepts only the observed `bare-user-only` repository mode, preserves
checksum-shaped symlink objects as opaque payloads, rejects structural links,
and runs real OSTree fsck before packing and after staged restoration. Inventory,
profile and digest checks precede destination publication. Bounds are8GiB logical
payload,2GiB compressed archive,100,000members and bounded subprocesses; these are
experimental limits, not approved cache allocations. Packing/fsck/archive bytes
and restore time are recorded. Local roundtrip fixtures exercise real tar/zstd
but mock fsck; actual hosted OSTree preservation is still unproven.

The fresh consumer skips ordinary builder-cache restoration for this experiment.
It prepares SDK state separately, restores the transferred cache first and compiler
results last, and forces application work without changing compiler inputs. A
PATH wrapper records the existing pinned builder action while preserving argument
and failure status. Require all dependency module hits, an actual application
rebuild, substantial compiler reuse and unchanged three validators. Measurement
uploads retain JSON and builder log without reuploading either large input archive.
The producer export's runtime test does not prove the separately rebuilt consumer
artifact; installed-profile qualification of that artifact remains necessary before
any shipping decision. Actual transport/reuse/retention and whole-job savings
must be established before adoption.


When both dependency transfer and runtime qualification are selected, only the
rebuilt consumer publishes `flatpak-runtime-repo`, after its validators pass.
The saved-profile job explicitly waits for that consumer and runs the unchanged
installed-artifact harness on its export. Ordinary runtime-only qualification
continues to use the producer export; an unselected compiler consumer must not
skip that job. Runtime reuse of the same run ID also selects this unique artifact.
This wiring closes the planned consumer-output coverage gap; actual hosted
saved-profile execution of that output remains unproven until the combined trial.


### V7: actual compiler reuse, package-setup timeouts retained

[Compiler consumer119865322923](https://github.com/obs-rust/obs-studio/actions/runs/39788434414/job/119865322923)
passed327hits/3misses, all6dependencyhits, a full application rebuild and all3
validators. Whole consumer418s; builder214s; compiler download11s; helper restore
1.158s; normal builder seed2,715,273,348B restored in42s. This establishes one
same-run artifact fresh-runner compiler reuse result, not retained production
cache transport or the routine warm cohort. Production cache publication and
runtime-export packaging were not part of that consumer clock.

The workflow ended cancelled: Qt exceeded30minutes and runtime infrastructure
installation exceeded25minutes, both during Ubuntu package downloads. The runtime
application/profile test never ran. All selected active jobs total194.6833
runner-minutes:21success,1failure,2automatic timeout cancellations; skipped jobs
excluded and no billing multipliers. No manual cancellation or unchanged retry.
See[portable v7 evidence](ci-evidence/2026-10-22-flatpak-transfer-v7.json).

### Actual OSTree preservation and v8 producer evidence

The local bounded container fixture passed real OSTree2025.7 commit/fsck before
and after archive restoration, same commit/ref, checkout data, symlink payload
and executable mode. This is a small real repository, not the hosted SDK/cache.
The first setup was rejected for a nested-manifest Dockerfile context before the
fixture ran; corrected setup501 and fixture502 passed. Raw identities and exact
fixture source are in[local evidence](ci-evidence/2026-10-22-flatpak-ostree-local.json).

Live[v8](https://github.com/obs-rust/obs-studio/actions/runs/39792586176) producer
119872197788 succeeded in899s, including optional experimental work. Actual
producer fsck passed10commits/19,089objects. Its cache-only archive retained70
opaque object links,19,367members and3,375,738,776 logical bytes; tar/zstd size
1,626,857,574B, published artifact1,626,859,156B. Packing including fsck took29.008s.
Compiler tar700,098,560B/published700,100,104B took3.045s to pack. Combined published
input artifacts total2,326,959,260B. These actual archives are not a minimal
dependency graph, production Actions-cache size measurement or retention proof.
Consumer119879152813 was queued at capture; restoration/reuse, consumer runtime
and full trial cost remain pending. See[producer snapshot](ci-evidence/2026-10-22-flatpak-transfer-v8-producer.json).
The prior section's statement that local fixtures mock fsck describes the unit
fixtures; the separate native fixture here now supplies additional real evidence.


### Qualified v8 consumer and projected production cache

Completed consumer119879152813 logs and its measurement artifacts prove restored
OSTree fsck, all6dependencyhits, real application rebuild,327compilerhits/3misses
and all3validators. Dependency helper restore25.902s; compiler restore1.411s.
Installed consumer runtime119883556478 passed saved screen/camera/microphone/browser
recording with310verifiedbrowserframes, modules, unchanged C++WebSocket and debugger
checks. App commit`dcec8109ee89db6fa5affbf9556d290ad39a2632e8fc879c1132bec1abaaa9a6`
and Debug commit`1efed457995d1935bd3b52e79aa1dfd6b4e658a9b318fa6935c3aeb3ed871e9c`
match the consumer export log. Synthetic/software coverage limits remain. Status
API lagged completed logs/artifacts; full selected-job cost is still pending.
See[qualified evidence](ci-evidence/2026-10-22-flatpak-transfer-v8-qualified.json).

The production change uses one existing Actions builder-family archive containing
only `.flatpak-builder/cache` and `.flatpak-builder/ccache`, with a versioned
architecture/manifest/source-generation key. All legacy restores are suppressed
on any new-state hit. A miss retains the old builder-then-compiler migration order.
The pinned action keeps ccache enabled and disables its automatic restore/save;
explicit master-only publication follows existing one-build ccache pruning and
all validators. The standalone compiler family becomes a migration input without
a new writer. Cold mode disables every production restore/publication.

Backend, family/global allocations and janitor policy stay unchanged. The actual
experimental pair2,326,959,260B supports this bounded projection; it does not prove
the exact future Actions archive size or retention. Inspect the first trusted
production publication and retained consumers after normal merge, then collect
the required routine warm before/after cohort. Neither the418s v7 consumer nor
v8's diagnostic consumer proves the parent's50%median target by itself.

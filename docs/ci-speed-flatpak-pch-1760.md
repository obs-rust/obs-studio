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

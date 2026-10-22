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

## Adoption hold

Real application freshness/reuse inside Flatpak, full-job warm/cold
measurements and the packaged screen/camera/microphone/browser/debugger scenario
are still required. Keep the PR draft until these are proven. This change alone
does not complete #1760's 50% whole-job target.

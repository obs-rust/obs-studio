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

## Adoption hold

Production SDK execution, real application freshness/reuse, full-job warm/cold
measurements and the packaged screen/camera/microphone/browser/debugger scenario
are still required. Keep the PR draft until these are proven. This change alone
does not complete #1760's 50% whole-job target.

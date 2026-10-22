# Flatpak performance: CEF debug processing

Tracked in [#1760](https://github.com/obs-rust/obs-studio/issues/1760).
This is the first candidate, not completion of the issue's full-job target.

## Measured bottleneck

[Full build 39721238636](https://github.com/obs-rust/obs-studio/actions/runs/39721238636/job/119635024523)
took 15m11s. Its logs place 162.4 seconds between CEF's debug-compression
message and stripping. Compilation, setup and export remain separate costs.
The manual extra rebuild in #1715 is not routine production work and is not
counted as a saving here.

Flatpak Builder compresses changed ELF debug sections before splitting them.
The OBS install copies CEF from its cached dependency into the final plugin
directory, so it receives that processing again. The dependency's existing
`no-debuginfo` setting alone does not suppress processing of the installed copy
by the later OBS module.

## Local ELF experiment

The exact pinned CEF archive was verified against SHA256
`8ffc64d06ea74da613eeda7f2aa5d4e98661af6fa3ed8c68724c9a6e4e89ea8e`.
Both local tasks ran through bosn with four CPUs. Each variant starts with the
same 2,031,350,672-byte ELF. Timings exclude extraction and copying inputs.

| Variant, one observation each | Compression | Splitting | Total | Separate debug bytes |
| --- | ---: | ---: | ---: | ---: |
| Current zlib-gnu |128.894s|2.057s|130.951s|420,562,136|
| No compression |0s|9.115s|9.115s|1,751,810,960|
| Zstandard |8.705s|1.961s|10.666s|394,021,568|

All allocated ELF sections and the build ID remain identical. GDB resolves
the same source line for `cef_version_info` in all three variants. All variants
also report the same 41,344 unavailable DWO references already present in the
input; this does not establish complete Chromium source-level debug coverage.
Tool versions, identities, GDB log hashes and observations are retained in the
[portable experiment](ci-evidence/2026-10-22-flatpak-cef-debug-experiment.json).

The uncompressed CEF option adds about 1.33 GB of debug data and could move
cost into exports or caches. It is not the selected CEF payload. Fast
compression is 120.3 seconds shorter in this local phase and produces a smaller
debug file, but that is not a demonstrated hosted Flatpak saving.

## Candidate and remaining qualification

Compress CEF's debug sections with `eu-elfcompress -t zstd` inside its immutable
dependency module, so that work can be reused from the dependency cache. Keep
that module's existing `no-debuginfo` setting: the final OBS installation still
extracts debug files at the correct installed path. Set the OBS module's
`no-debuginfo-compression` option to prevent conversion back to zlib. Its
first-party debug information remains available for normal splitting/export;
the candidate does not disable debug extraction or enable destructive stripping.

This follows the distinct compression and extraction flags in
[Flatpak Builder's post-processing implementation](https://github.com/flatpak/flatpak-builder/blob/bd35975d1b089960251809a5bec738ded3cd6f46/src/builder-module.c).
The exact production SDK (elfutils 0.195) processes the pinned CEF with the
same allocated sections/build ID and source-line lookup. Full export
[39754042722](https://github.com/obs-rust/obs-studio/actions/runs/39754042722)
passes existing Flatpak validators. Its complete Flatpak job takes 12m04s,
including optional runtime-export upload; that is one observation with changed
source/dependency cache state, not a comparable warm-job speedup.

Installed-package qualification
[39763463067](https://github.com/obs-rust/obs-studio/actions/runs/39763463067)
reuses that immutable export and passes app/Debug build IDs, GDB source lookup,
authenticated unchanged WebSocket v5, all four saved sources, clean shutdown
and module loading. The 12.033-second recording passes screen/camera motion and
continuous 440 Hz microphone tone checks. All 302 checked browser frames render
the green HTML patch. Browser software rendering is explicit; the preceding
default-hardware fixture failed on a llvmpipe/no-DRM runner. Physical devices and
GPU rendering are not covered, and the upstream missing DWO limitation remains.
Portable identities, logs and the six qualification attempts are recorded in
[the runtime qualification PR](https://github.com/obs-rust/obs-studio/pull/1771).

These observations qualify this debug-processing change's installed artifact;
they do not establish warm application-cache reuse, production publication cost,
global retention, cold performance or the whole-issue speedup. The new combined
configuration has its own full qualification run before further adoption.

The full goal remains at least 50% lower comparable warm job time, with cold
and warm observations, required validators, complete application functionality,
safe incremental compilation and an honest cache budget. Compilation/PCH reuse
is a separate candidate; no Rust compiler-wrapper safeguard changes here.

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
Actual Flatpak SDK tool support, OSTree export and transferred bytes, packaged
debugger behavior, browser loading, and a saved screen/camera/microphone profile
remain to be qualified. Focused manifest regression tests are not substitutes
for those checks.

The full goal remains at least 50% lower comparable warm job time, with cold
and warm observations, required validators, complete application functionality,
safe incremental compilation and an honest cache budget. Compilation/PCH reuse
is a separate candidate; no Rust compiler-wrapper safeguard changes here.

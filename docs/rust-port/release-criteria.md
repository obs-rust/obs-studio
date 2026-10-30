# Rust implementation release criteria

This is the decision for #141. A port is **complete** when the applicable
four testing tiers in [testing-policy.md](testing-policy.md) pass. It is
**shipped** when a release artifact uses that Rust implementation by default.
The normal developer/application build now links and installs the hybrid
`libobs`: completed Rust ports plus unported legacy C/C++. The C reference is
explicit testing infrastructure (`OBS_BUILD_C_REFERENCE=ON`), excluded from
normal builds and installation. `OBS_LIBOBS_BACKEND=C` is temporary rollback;
product identity (`OBS_BRAND_RUST`) does not select an implementation.

## Releasing the hybrid default

There is no manual approval file and no manual evidence review (#1818). A
release is published when the maintainer dispatches it and the automated CI
gates pass:

- **Nightly:** the daily `release-rust.yaml` run publishes a GitHub
  **pre-release**, the rolling `continuous` AppImage and the nightly update
  feed. Nightlies always build the hybrid backend (`OBS_LIBOBS_BACKEND`
  default `HYBRID`); the packaging step fails if Rust is not linked into the
  production libobs. Pre-releases are never Latest, so `releases/latest` and
  the stable feed never offer them.
- **Public (blessed) release:** happens only when the maintainer dispatches
  `release-rust.yaml` with `certify=<tag>`. That dispatch is the decision to
  release; the `certify` job then runs only its automated checks
  (`check-certifiable`) before marking the release Latest and updating the
  stable feed. Agents never dispatch certify.
- **AppImage tag push:** attaches to the tag's release after the AppImage
  build and its automated tests pass.

Every automated gate stays: the builds, Tier 2, the packaged-app smoke tests
and `check-certifiable`. The `OBS_LIBOBS_BACKEND=C` rollback stays available.

If something is broken, fix it forward fast with a new release. A failure a
release let through is a coverage gap: close it by adding an automated test
that would have caught it, not by adding a manual review step.

### Coverage targets without automated gates yet

The checks below were once manual release criteria. They are goals for new
automated tests, not release blockers:

1. **Compatibility:** Tier 2 with OFF and ON on Linux, macOS and Windows, x64
   and arm64, plus identical exported public libobs symbols.
2. **Running application:** the unchanged obs-websocket Tier 4 suite against
   the release artifact; a full-app launch on every release platform.
3. **Performance:** hot-path benchmarks within 10% and the full-app recording
   workload within 5% of C (#142), checked automatically per release.
4. **Stability:** multi-day crash-signature tracking of nightly artifacts.

## Record C/Rust benchmark evidence (#142)

The Criterion suite lives in the test-only `obs-c-oracle` crate. It compares
production C ABI shims with the original C sources, compiled in the same
optimized benchmark executable. Run the fixture and ownership checks first:

```sh
soldr cargo bench -p obs-c-oracle --bench port_comparison -- --test
```

Record five complete runs and their median times, run-to-run ranges, and
per-run confidence intervals:

```sh
uv run rust/tools/benchmarks/record.py --cpu 7 --output baseline.json
```

`--cpu` pins the runner on Linux; omit it on systems without that affinity
API. The report records the CPU, affinity, OS, Rust/LLVM and C compiler
versions, relevant environment flags, command, and a SHA-256 fingerprint of
the benchmark and implementation sources. The recorder refuses incomplete
paired measurements and source changes during measurement. Criterion's raw
samples remain under `target/criterion/<benchmark>/<baseline>/`.

A full run needs every category in `record.py`'s `CATEGORIES`; the bench
units of the ports add their own groups, which are recorded too. For the
before and after numbers of one kernel (#3009), `--category` (repeatable)
runs and requires only the named categories:

```sh
uv run rust/tools/benchmarks/record.py --cpu 7 --category format --output format.json
```

The SIMD candidates of #3009 are categories of their own: `format` (each
format conversion on a 1080p frame, against the shipped C and against the
same C on the plain-C SSE stand-ins), `volmeter` (sample and true peak of
one tick) and `audio_mix` (the mix add, and one output tick with the clamp
of 6 mixes).

The suite covers all exported bmem and vec2/3/4 shims, CRC at 64/4096/65536
bytes, dstr concatenation/safe formatting/replacement at 128/4096 bytes,
and the AVC/HEVC/AV1 parser entry points on structured synthetic packets.
AVC start-code cases scan a 256/16384-byte prefix. Packet comparisons include
the allocation's reference-count prefix and metadata. Allocating batches
hold at most 64 live outputs; allocation/free and reallocation have separate
timings. Setup and output cleanup for batched string/reallocation operations
are excluded, while codec timings include output cleanup.

Variadic `dstr_printf` remains C in both OFF and ON. Its two control cases
are labeled `printf_control` and excluded from Rust regression decisions.
The C string and codec oracles use the shared libobs allocator; bmem cases
compare the independent original C allocator with the Rust allocator. The
standalone oracle uses Cargo's optimized C build, without the full OBS C IPO
configuration. These measurements describe shim costs and synthetic inputs;
they do not establish full-application performance or caller inlining.

The report flags medians above the existing **1.10 Rust/C time ratio**. That
classification is descriptive, not a claim of statistical significance.
Inspect confidence intervals and run-to-run variation, and reproduce on
release hardware before making a default-flip decision. The 5% recording
workload bound and the other release gates above still apply.

### Recorded baseline

The [2026-10-14 baseline](benchmarks/baseline-2026-10-14.json) records five
runs of 70 paired comparisons on a Ryzen 7 3700X, pinned to CPU 7, using
Rust 1.98.1 and GCC 15.2.0. Twenty-one measured medians exceed the 10%
bound. The largest is `dstr/safe_printf/4096`: 59.47 times the C median,
with paired run ratios from 57.96 to 67.83. These results identify work
needed before the default flip; they do not satisfy its performance gate.

### Cross-language LTO investigation

The Linux x86_64 ON probe used Rust 1.98.1 (LLVM 22.1.8), Clang/LLD
22.1.2, ThinLTO on C/C++, and Rust linker-plugin LTO. This follows the
[rustc cross-language LTO instructions](https://doc.rust-lang.org/rustc/linker-plugin-lto.html).
With the usual OBS Linux build dependencies installed, reproduce with:

```sh
printf '#!/bin/sh\nexec soldr cargo "$@"\n' > /tmp/obs-cargo-soldr
chmod +x /tmp/obs-cargo-soldr
export RUSTUP_TOOLCHAIN=1.98.1
export RUSTFLAGS='-Clinker-plugin-lto -Crelocation-model=pic'
cmake -S . -B build-lto -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER=clang-22 -DCMAKE_CXX_COMPILER=clang++-22 \
  -DCMAKE_C_FLAGS=-flto=thin -DCMAKE_CXX_FLAGS=-flto=thin \
  -DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=lld-22 \
  -DCMAKE_SHARED_LINKER_FLAGS=-fuse-ld=lld-22 \
  -DRust_CARGO=/tmp/obs-cargo-soldr \
  -DENABLE_FRONTEND=OFF -DENABLE_UNIT_TESTS=ON \
  -DENABLE_BROWSER=OFF -DENABLE_WEBSOCKET=OFF -DENABLE_AJA=OFF \
  -DENABLE_WEBRTC=OFF -DENABLE_NVENC=OFF -DENABLE_FFMPEG_NVENC=OFF \
  -DENABLE_QSV11=OFF -DENABLE_VST=OFF -DENABLE_SCRIPTING=OFF \
  -DENABLE_VLC=OFF -DENABLE_NEW_MPEGTS_OUTPUT=OFF
cmake --build build-lto --target libobs -j4
```

The stock configuration stops on Clang warnings treated as errors:
double promotion in `obs-data.c` and a missing format attribute in
`graphics.c`. To investigate the linker independently, the experiment
added `-Wno-error=double-promotion -Wno-error=missing-format-attribute` to
both C/C++ flag strings. This reached the shared-library link, which failed:

```text
ld.lld: error: relocation R_X86_64_TPOFF32 against thread_graphics
cannot be used with -shared
```

The linker command included `-fPIC` and `-shared`; inspected C and Rust
bitcode both recorded PIC level 2. Repeating with
`--target=x86_64-unknown-linux-gnu` in both C/C++ flag strings to match
Rust's target triple produced the same relocation failure. Its cause is
unresolved. Cross-language LTO is therefore not enabled by this change,
and no LTO performance improvement or successful full ON build is claimed.
The warning downgrades were confined to the probe. A future LTO change
must resolve the build failures and pass the normal compatibility gates
before its performance can be compared with this baseline.

## Record full-app recording evidence (#1305)

The 5% recording bound needs runs of the real application on release
hardware, not shim benchmarks. Use a hardware renderer (never llvmpipe or
Xvfb), one saved scene and profile, the same build for both backends (the
explicit C reference or installed hybrid `libobs`, selected by the
library path), and no profiler in the timed runs.

1. Before each run, OBS must be running with the backend under test. The
   run checks the OBS log and the mapped libraries first
   (`ci/recording_evidence_check.py`) and refuses a software renderer, the
   other backend or libraries from outside `--prefix`.
2. Record one run:

   ```sh
   uv run rust/tools/benchmarks/recording.py run --log <OBS log> --pid <OBS pid> \
       --libobs rust --prefix <install prefix> --encoder obs-nvenc --seconds 120 \
       --ws-config <obs-websocket config.json> --delete-recording --output rust-1.json
   ```

3. Restart OBS with the other backend and record again. Interleave the
   backends (C, Rust, C, Rust, ...) until each has at least five runs.
4. Compare:

   ```sh
   uv run rust/tools/benchmarks/recording.py report c-*.json rust-*.json --markdown report.md
   ```

The report refuses runs with different conditions (video settings,
recording profile, scene, source version, renderer, graphics module) or
two builds of one backend. It gives the median and range of the frame
render time, CPU, and the render, output and encoder skip rates per
backend, and the Rust/C ratio. Exit code 2 means the gate failed. Each run
JSON also keeps the raw per-second samples and the encoder log lines.

## Retire the C implementation of a unit

Keep the original C file while reference testing and temporary rollback need it.
After one stable hybrid release, a separate PR may retire the
C fallback for a unit only when the supported C rollback build remains linkable
through another implementation or the C rollback option itself is retired. That PR
must show the unchanged C tests, public symbol/layout checks, and Tier 3
parity on the supported configurations. Move the original C source to
`rust/obs-c-oracle/legacy/` in the same PR, update the oracle wrapper to use
it, and keep differential tests running. Preserve the file's upstream
copyright and license notices. Removing a production C file must not silently
remove the behavioral baseline used to test its Rust replacement.

## Certified releases (in-app updates)

Every daily beta from `release-rust.yaml` is published as a GitHub
**pre-release**, after the release gate and every platform build pass. It is
never marked Latest, and the in-app updaters never offer it.

A **certified** release is a daily beta a maintainer promotes to a full
release:

```sh
gh workflow run release-rust.yaml -f certify=obs-studio-rust-v<version>
```

The `certify` job accepts only a published `obs-studio-rust-v*` release
(published only after the gate). Its "Check the release" step runs
`ci/update_feed.py check-certifiable` (#1757), which refuses the tag unless:

- the `release-rust.yaml` run that published it (the newest run for the
  tag's commit whose "Publish pre-release" job succeeded) concluded success,
  with every job successful. Only the certify job and a reusable workflow's
  own jobs (`Release gate 🚦 / ...`, `AppImage 📦 / ...`) may be skipped;
- the release has been out at least `min_age_days` days (default 2), so a
  daily gets two days of use as a beta first. For an emergency,
  `-f min_age_days=0` skips the age check and logs a warning.

It then checks its assets against `SHA256SUMS`, and
runs `ci/update_feed.py`. That script writes the Sparkle appcasts
(`updates_<arch>_v2.xml`, DMGs signed with `OBS_RUST_SPARKLE_PRIVATE_KEY`),
the Windows `manifest.json` (ZIP URL, SHA-256 and size) and `branches.json`,
both with `.sig` files signed by `OBS_RUST_UPDATE_RSA_PRIVATE_KEY`. The job
uploads them to the release, and only then clears the pre-release flag and
marks it Latest. Both updaters read
`https://github.com/obs-rust/obs-studio/releases/latest/download/`, which
GitHub resolves to the newest full release. Certify a build only after a
maintainer has run it. Certifying an older release makes it Latest again,
but an app newer than that release won't downgrade.

### Stable, nightly and certify

| Channel | Who gets it | Feed | Written by |
|---|---|---|---|
| `stable` (default) | everyone | `releases/latest/download/` (the Latest release) | the `certify` job, on a maintainer's dispatch |
| `nightly` (opt-in) | users who pick **Nightly** under Settings > General > Updates > Update Channel | `releases/download/continuous/` (the rolling `continuous` pre-release) | the `nightly_feed` job, after every published daily |

1. **Daily:** each day `release-rust.yaml` publishes `obs-studio-rust-v<version>`
   as a pre-release once the gate and every build pass. Its `nightly_feed`
   job then runs `ci/update_feed.py build --channel nightly` and uploads the
   result onto `continuous`: Sparkle appcasts whose items carry
   `<sparkle:channel>nightly</sparkle:channel>`, and a `manifest.json` (+
   `.sig`) with `"channel": "nightly"`, all naming the daily's own assets. It
   is signed with the same two secrets. Without them the job skips with a
   warning and the daily still succeeds (#1759).
2. **Certify:** at least two days later, a maintainer runs
   `gh workflow run release-rust.yaml -f certify=<tag>` (add
   `-f min_age_days=0` only in an emergency). The gate above checks the run
   and the age, then the stable feed goes up and the release becomes Latest.
3. **In the app:** the update channel lives in the global config
   (`[General] UpdateBranch`, default `stable`). `nightly` is built into the
   app, next to `stable`. Any other value (an upstream `beta`) falls back to
   stable. On Windows the updater reads `manifest.json` from the channel's
   feed. On macOS the Sparkle delegate reads the channel's appcast and
   allows only that channel, so a stable app never takes a nightly item.
   Switching from nightly back to stable never downgrades: the app waits for
   a certified release newer than the nightly it runs. Every feed and
   download URL must be under `https://github.com/obs-rust/obs-studio/`
   (`ci/test_no_upstream_updater.py`).

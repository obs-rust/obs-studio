# Batch 2 native graph rollup

Refs #1390. The original integration head `3c000f1e6` starts at `84bf218a5` from #3832 and preserves, in
order, the complete histories of `test/1390-b2-libobs-af4`,
`test/1390-b2-libobs-go5`, `test/1390-b2-libobs-pz4`,
`test/1390-b2-rust-static5`, #3829 and #3830. The landing PR is based on master after #3832 merged. All slice reports remain beside
this file, including their historical evidence and limitations.

The assembled graph has **504 targets and 504 test entries**, adding 59
executables to #3832: 30 real-libobs and 29 Rust-static. All 71 registration
checks pass in the existing Application binary. The eight Rust test categories
are unchanged. No existing C test, public C header, WebSocket file or workflow
is edited.

## Integration repairs

- Preserve both conflicting graph regions and sort all target/test entries.
  At the original integration head every requested tip is an ancestor, including every original RED/GREEN
  commit. Compare each slice's changed records against the final graph; only
  the explicit pthread repairs below intentionally change a slice's record.
- Declare conditional `OBS::w32-pthreads` for async delay and both scalar audio
  filters: their included filter PCH imports `util/threading.h`. Preserve strict
  float flags, link order, direct execution and absent timeout properties.
  RED `59ae902b7` demonstrates all three missing dependencies and the SIMDe
  fixture assumption; GREEN `a9443f441` repairs them.
- The selected standalone core-header fixture resolves header-only SIMDe using
  the repository's CMake finder for both consumers, with no SIMDe pkg-config
  lookup. The inherited glyph target also uses `SIMDe::SIMDe` instead of a
  `simde` header-package lookup (RED `95e2e83c3`, GREEN `c23d4d48e`).
  Package-disabled host tests still return before resolving headers.
- Reconcile the dual-core inventory against the entire evaluated Linux core
  graph, rather than retaining stale conflicting constants. With production
  defaults and a Rust archive: 422 tests with PulseAudio and HEVC, 417 without
  PulseAudio, and one fewer with HEVC disabled. The existing archive-member
  count stays independent (31 total). A strict independent inventory regression
  checks all four configurations. Declare `HAVE_OBS_OPENGL=false` in the Linux
  core-test builder, alongside the unavailable FFmpeg/QSV plugin facts, so the
  existing guarded OpenGL test uses a known requirement name.

## Local evidence

- 71 exact registration checks: final `local-test-hXu9l3.log` (earlier
  `local-test-vTHKos.log`); raw results and replay
  sources are in `/tmp/b2-integrate.XXXXXX/`.
- 16 Python graph ordering, shared-list, slice, category, nextest-budget,
  cmocka and portability checks pass. Scoped rustfmt passes.
- Whole-graph legacy replacement and 11 other graph contracts pass:
  `local-test-AyEJc9.log`. The additional Qt-metaobject contract cannot find
  Qt6Core through its initial pkg-config environment; that combined invocation
  exits 101 and is not reported as an overall pass.
- Final Linux Clippy: `local-test-6531WI.log`; final Windows-target Clippy:
  `local-test-kQhxtc.log`. Both exit 0 with `-D warnings`, all affected crate
  targets and `libobs-integration`. Cognitive complexity remains enforced with
  no threshold change or new allowance.
- Build the production Rust archive through soldr: `local-test-LJ1wHv.log`.
  Actual Cargo/cc::Build replay of all 59 additions with GCC and Clang:
  `local-test-p3jGh0.log`, `local-test-XwZNHz.log`. Each reports **56 pass,
  2 runtime failures, 1 link failure**, returning 101 rather than hiding them.
- Actual canonical CMake/CTest replay: `local-test-7ReBny.log`; focused replay
  after the archive finished building and the happy-eyeballs fixture imported
  its real POSIX include interface: `local-test-wHqWNo.log`. Combined results
  likewise give **56/59 passing for each compiler**. CTest inventories and every
  command status are retained with the replay sources. This supplies fresh GCC
  and Clang evidence for all a-f additions, including their real library links.
  All 116 linked real-core artifacts (29 targets, two consumers, two compilers)
  resolve `libobs.so.30` to the selected installed core; loader observations are
  retained in `loader-results.json`.

The three failures in each Cargo/CMake compiler combination are unchanged C
tests against installed OBS 32.1.2: `test_image_file` cannot link the newer
`gs_image_file_ex_*` exports; `test_image_exif_orientation` fails 21 assertions;
`test_remux_audio_timestamps` fails four cases. The selected FFmpeg headers and
libraries also differ from the installed core's major versions. This external
library smoke does not decide final-head hybrid/C-reference parity. Do not
weaken these tests or claim that the complete runtime inventory is GREEN.

Native macOS/Windows runtime, final-head hybrid/C-reference execution and exact
exports, full workspace lint/unittest, application/profile/protocol and hardware
validation remain unverified. No hosted runs, target pruning or soldr gc were
requested. Full unittest was not invoked because its wrapper prunes targets.

Additional inherited runtime contracts (`local-test-9aKYFI.log`) run with both
GCC and Clang: ten contracts per compiler pass, including the production
Rust-static decision, header, monitor, source and transition-start suites, both
inherited archive suites, plugin helpers, Pulse decisions and product/meter
helpers. The combined 24-invocation orchestration returns 1 because the utility
and Qt remux-queue contracts fail for each compiler. Initial utility replay
lacks FreeType pkg-config metadata; with the normal SDK metadata restored,
`local-test-gEvLUp.log` exposes the glyph SIMDe assumption fixed above. Final
`local-test-MWx3xS.log` passes that lookup/compilation and reaches the unrelated
missing XCB package. Full 27-target standalone replay therefore remains
unverified. The remux-queue fixture fails to link the installed Qt 6.11.2 against
its required PCRE2 10.47 symbols. These broad SDK/environment failures are
retained; they do not count as passing contracts. The focused new 59-target
replays are separate and complete as reported above.

## Landing on master

The initial per-commit rebase stopped at the first legacy registration conflict.
The combined batch-only delta from `84bf218a5` to `3c000f1e6` was applied with
three-way merge to master `335f2839f`; all 59 additions and all master records
are retained. The original RED/GREEN commits remain in the pre-landing branch
history recorded above. Master adds two other targets (one core, one graphics),
so the total is now 504 and the evaluated core count increases by one.

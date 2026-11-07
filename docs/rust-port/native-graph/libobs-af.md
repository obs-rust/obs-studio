# Scalar audio filter native graph (batch 2, libobs-af)

Refs #1390. Based on #3832; this is a bounded two-target addition, not completion
of the alphabetical slice or the broader native parity issue.

Register unchanged `test_dynamics_threshold` and `test_expander_unity` once for
Cargo and CMake. Both call production scalar filter code without OBS startup,
graphics, an audio device or hardware. Preserve real `OBS::libobs` linkage,
`m` only outside MSVC, `/fp:strict` on MSVC and `-fno-fast-math
-ffp-contract=off` otherwise. Both remain unconditional, with a direct command
and no timeout or wrapper. Track the included filter implementations, forwarding
source and PCH through the filter directory watch input. Windows pthread headers
and linkage come from the real libobs public interface; no package lookup is
added. Existing C tests and public headers are unchanged.

RED `f5537598e` fails `canonical_audio_filter_scalars_are_registered` with
`missing scalar audio filter`. GREEN registration, Linux/macOS/Windows/BSD
selection, whole-graph legacy replacement and the focused Python graph,
sorted-list, Windows diagnostics and build-group guards pass.

GCC and Clang each compile, link and run both executables through actual Cargo
and CMake consumers against installed OBS 32.1.2. Each dynamics run reports
1,507,328 checks; each expander run reports 360,744 checks, all with zero failures.
Cargo loader inspection confirms the selected real libobs. Both CTest inventories
contain exactly these two targets with no timeout/wrapper; generated compile
flags retain both non-MSVC strict floating-point options. This external-library
smoke evidence does not prove current-head hybrid/C-reference runtime or export
parity. The rollup must obtain that validation. Native Windows/macOS runtime,
application/profile/protocol paths and other a-f tests are not claimed tested.

Test cost: 25 reduced debug registration runs, longest 0.213 s, peak RSS
12,028 KiB, no test-owned threads. The existing Application category binary is
used; no Rust test executable is added. Independent clud-review found no issues.

`test_async_delay_pacing` is excluded by assignment (#3829). `test_audio_io`
requires OBS startup/display and its original xvfb wrapper. Graphics tests,
ALSA/capture/CoreAudio and Qt/frontend host cases require separate environments;
Rust-static cases belong to another batch slice. Other dependency-bearing
effect-parser, FFmpeg and file-updater candidates remain unconverted here;
this branch makes no exhaustive eligibility claim for them. No new CI run or
PR is requested: the pool manager collects this branch into the batch rollup.

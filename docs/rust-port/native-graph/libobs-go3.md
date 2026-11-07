# Real-libobs graph continuation: Happy Eyeballs

Refs #1390. Branch `test/1390-b2-libobs-go3` continues from
`test/1390-b2-libobs-go2` at `b6cf7fcddcd75914ecc97574c50a55cdb73ba070`.
This is a branch-only pool handoff; no PR or hosted CI was requested.

Register the unchanged `test_happy_eyeballs` (five cases) and
`test_happy_eyeballs_gai_error` (one case). Preserve real `OBS::libobs`,
`OBS::happy-eyeballs`, cmocka, Windows `ws2_32`, direct invocation and the
absence of timeout/skip/resource properties. The original target-presence guard
becomes `HAVE_HAPPY_EYEBALLS`, derived from the actual CMake target or Cargo
dependency interface. No display, graphics or physical device is needed.
The connector performs real loopback connections and reserved `.invalid` lookups.
No C tests, public headers, WebSocket files or workflow files changed.

## Evidence

- RED `e16497f51`: the new Application-category registration check fails with
  `canonical graph omits test_happy_eyeballs` (`local-test-jNO9KE.log`).
  GREEN registration passes (`local-test-mUjKIA.log`). No new repository Rust
  test executable was added.
- GCC and Clang each compile and execute both tests through the actual Cargo
  graph and CMake graph consumer: all eight command statuses are zero
  (`local-test-pAj2Qu.log`). Six unchanged cmocka cases pass per consumer/compiler;
  `ldd` verifies the selected installed OBS 32.1.2 core for Cargo artifacts.
  The connector archive uses the production unity source, with no stubs.
- Both compilers also build and run the original CMake fragments against the
  production connector target (`local-test-uNrjC9.log`), six cases each.
- Original/canonical CMake inventories and timeout/skip/resource properties
  match for Linux, Windows, macOS, FreeBSD and OpenBSD, with the connector
  present and absent (`local-test-y0wbO3.log`). This is simulated selection,
  not native Windows/macOS runtime evidence.
- Registration cost: 25 reduced debug runs, longest 0.0222 s, peak RSS
  10,700 KiB, no test-owned threads (`local-test-rPSOY5.log`). The unchanged
  connector itself creates its existing worker threads in native execution.
- Binary-budget, native graph slicing and sanitizer-harness Python guards pass
  (`local-test-4AofNQ.log`). One primary read-only clud-review found no defects
  and independently reran the registration test successfully.
- Scoped Application-category and obs-build Clippy passes with
  `libobs-integration` and `-D warnings` (`local-test-Ink5LP.log`). Cargo fmt
  for the Application category also passes (`local-test-zm2W5z.log`).
- The final formatted source compiles and all three inherited/current libobs
  g-o registration modules pass (`local-test-cxo0iR.log`), including go2's
  previously uncompiled exact-source assertions. Whole-graph legacy replacement
  also passes (`local-test-G5Jton.log`).

The replay uses an installed real core, not a final-head hybrid/C-reference
pair. Exact export parity, native Windows/macOS execution, application/profile,
protocol and hardware coverage remain unverified here. Full lint/unittest was
not run; target pruning and soldr gc are prohibited for this assignment.
An initial direct Soldr rustc replay failed on a missing broker daemon path;
the documented wrapper setting then stalled. A scratch Soldr Cargo build
succeeded and produced the replay used above. No lock was removed or guard
bypassed. Scoped Clippy initially refused an active build lock while that
scratch build was running; the ordinary retry passed after the build completed.

## Remaining scope

The inherited literal g-o real-libobs inventory has 47 remaining entries after
these two additions. It is not an evaluated configuration inventory. The prior
`libobs-go2` handoff lists each entry and its reason; all reasons remain except
that these two connector tests are now covered. Graphics/host integrations,
display/session services, FFmpeg playback fixtures, Rust-static linkage, TLS
host composition and output/Qt integrations remain separate work. No broader
parity completion is claimed.

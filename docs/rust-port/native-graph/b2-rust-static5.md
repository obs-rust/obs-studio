# Batch 2: Rust-static transition start

Refs #1390. Branch `test/1390-b2-rust-static5` continues from
`test/1390-b2-rust-static4` at `f47b5c4485037cf575e6c2a92ed1561827c55d92`.
This slice registers `test_transition_start_steps_rust`, preserving the unchanged
C source and its 17 cmocka cases. No implementation port or public API changes.

The target retains archive-first/cmocka-second linkage and archive availability
on both target and test. It imports libobs's header interface and the portable
`SIMDe::SIMDe` header target, without linking the core or supplying stubs.
The direct command, absent timeout/skip/wrapper/sanitizer settings and original
lack of host/math guards are preserved. Its legacy fragment becomes a graph
reference; no C test, public header or WebSocket file changes.

The independent registration module joins the existing Application binary.
Its integration-gated Linux replay builds and runs the real production archive
through both Cargo and CMake, obtains SIMDe through the repository CMake finder,
and configures the production obsconfig template. CTest inventory checks archive
availability OFF/ON and the absence of a TIMEOUT property. Cargo checks that the
production archive is the only dependency artifact. No pkg-config SIMDe lookup
or platform requirement is introduced by this slice.

## Evidence

- RED `ab19ba51c`: focused registration check fails with
  `missing Rust-static transition start`; exit 101, `local-test-eqnYtN.log`.
- GREEN: real GCC Cargo/CMake replay, two Rust checks pass,
  `local-test-bVjHkL.log`; Clang equivalent, `local-test-WhbtWy.log`.
  Each consumer runs the unchanged 17-case executable. Existing C initializer
  warnings remain unchanged.
- Static-archive, graph-slice, graph/shared-list ordering, eight-category,
  sanitizer harness, Windows diagnostic and uv-invocation guards pass:
  `local-test-uaqUiB.log`.
- Whole-graph legacy replacement passes: `local-test-4jDKwR.log`.
- Scoped Application Clippy with libobs-integration exits 0:
  `local-test-QRQXDN.log`. No complexity allowances or threshold changes.
- Test cost: 25 reduced debug registration observations, longest 0.034 s,
  peak child RSS 15,004 KiB, no own threads. The real compilation contract uses
  the existing native-build budget exception and compiler/CMake subprocesses.

Logs are under `/home/niteris/.clud/tmp/tool-output/local-test-private/`.
No hosted CI, full workspace gate, native Windows/macOS runtime, final-head
hybrid/C-reference OFF/ON export parity or application/profile/protocol validation
is claimed. The manager owns rollup admission; this does not complete #1390.

## Deferred inventory

The predecessor's remaining 26 candidates stay deferred: one Pulse monitor
header/host contract (Linux/FreeBSD option guard; awaiting cross-host graph
requirement handling), two OS/sanitizer contracts (window match and Windows
keys), thirteen real-core/private-adapter build contracts, nine real-core
linkage/header contracts and one graphics/display runtime. The predecessor
report lists their names. These categories describe uncompleted scope, not
proof that those tests cannot be migrated. No other candidate's registration
or policy is changed in this slice.

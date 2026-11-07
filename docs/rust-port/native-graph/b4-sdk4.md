# Batch 4: FreeType file SDK fixture

Refs #1390. Branch-only pool assignment, based on
`origin/test/1390-b4-sdk3` (`9840350caa06a7a34a8641698494a75beeefaa21`).
The manager owns the rollup PR and hosted merge gate.

Register the unchanged `test_text_aa_file` executable (two cases) in the
shared graph. Preserve its Linux/`OBS::text-freetype2` owner guard, seven
production sources, real FreeType/Fontconfig/libobs/cmocka linkage, fixture
font definition, direct command and unset timeout. Its existing C fixture
interposes graphics allocations; this change adds no stubs and needs no
display. Optional SDK discovery is QUIET and Linux/owner bounded. The main
Cargo builder does not build the text plugin, so the new owner/SDK facts
default false; the explicit SDK contract enables them.

The graph grows from 518 to 519 targets and tests. Every existing record is
unchanged. No existing C tests, public headers, WebSocket files, workflows or
Rust test binary counts change.

## Evidence

- RED `ef1e6fee9`: registration and selection assertions fail because the
  target is absent (`local-test-LpOFbK.log`).
- GCC: all four contracts pass, including both unchanged cases through
  Cargo/cc::Build and again CMake/CTest (`local-test-9LzO1o.log`).
- Clang: the same four contracts and unchanged cases pass through both
  consumers (`local-test-9eKQVQ.log`).
- CMake selection: enabled=1; non-Linux=0; owner-off=0; FreeType-absent=0;
  Fontconfig-absent=0. Direct command and unset timeout checked. Package-free
  configure passes Linux/FreeBSD/OpenBSD/Windows/macOS selections.
- Seven focused Python graph, cmocka and test-budget modules pass
  (`local-test-CbjETq.log`). Managed rustfmt and gersemi 0.25.0 pass.
- Test cost, 25 reduced-debug runs each: registration longest 0.0292 s,
  selection 0.0473 s, package-free configure 0.3850 s; peak child RSS
  19,368 KiB. Fast contracts own no threads. Native compiler/CMake replay
  retains the existing native-build budget exception. Measurements are in
  `build_ci_audit/b4-sdk4/test-cost.json`.

Logs live under `/home/niteris/.clud/tmp/tool-output/local-test-private/`.
The SDK runner is retained at `build_ci_audit/b4-sdk4/run.py`. Installed OBS
32.1.2 supplies real runtime smoke evidence, not final-head hybrid/C-reference
parity. On this Nix host, the local runner supplies real FreeType and
Fontconfig development/library prefixes to CMake, derived from pkg-config.

## Deferred work

This is a focused slice, not an exhaustion claim. Of the original 41
SDK/owner fragments, 13 are now registered and 28 remain. Those include seven
excluded runtime fixtures, two PipeWire linked GLAD/GIO candidates, the
V4L2 fd-cloexec candidate requiring final-head core behavior, two font
fixtures needing generated Fontconfig files not modeled by the graph, and
16 other SDK/plugin fragments (V4L2, Wayland hotkeys/portal, Qt, Python,
QSV/VPL and VST). The original other exclusions remain 30 graphics fixtures,
seven WebSocket, two X11 display and one services-plugin/display: 68 of the
original 81 fragments remain deferred/excluded.

Full workspace unittest, final-head dual-core Tier 2 and export parity,
native Windows/macOS/BSD execution, application/profile/protocol/device and
desktop behavior remain unverified. No PR, hosted CI or auto-merge is created
for this branch-only assignment.

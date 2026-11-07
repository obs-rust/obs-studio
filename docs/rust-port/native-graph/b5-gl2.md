# Batch 5 follow-up: XComposite error replies

Refs #1390. Branch-only slice `test/1390-b5-gl2`, based on
`origin/test/1390-b5-gl` and successfully rebased onto updated
`origin/test/1390-b4-rollup` at `c8f429ec2` (rebase exit 0).
The previous visibility slice remains intact.

Register `test_xcomposite_error_replies`, two unchanged C cases, preserving
Linux, owning `linux-capture` target and real `OBS::libobs-opengl` guards;
the Linux capture unity source; all original libobs, glad, X11/XCB and cmocka
linkage; renderer artifact definition; runtime working directory; direct
command and 60-second timeout. The fragment retains original required SDK
discovery under the original guard. `HAVE_LINUX_CAPTURE` is explicitly
initialized on every CMake host and in the Rust native fact table, so eager
requirement evaluation succeeds on Windows and macOS. The native compatibility
builder does not build the owning plugin and defaults its fact to false;
the explicit real-SDK replay supplies that owner and its dependencies.

The two XComposite cases exercise real XCB error replies and allocation growth
without registering an OBS source or encoder. They therefore do not encounter
the current-header/installed-core ABI mismatch below. They retain the original
renderer dependency even though these two cases do not initialize core video.
The combined replay also runs the prior eight visibility cases, which initialize
real OpenGL through the matching installed renderer under private Xvfb.

## Rejected candidates

All five candidates below built through their original CMake registrations
with real dependencies and installed OBS 32.1.2. No existing test was edited.
The original registrations remain unchanged for every rejected candidate.

- `test_preview_only_hide`: all six cases fail because installed libobs rejects
  source-info size 424 (supported 408), then cannot create the required sources.
- `test_transform_clipboard`: CTest reports Passed, but source registration
  fails with the same size mismatch and every box source is missing. A passing
  status without the required objects is not usable evidence.
- `test_replay_buffer_naming`: CTest reports Passed, but output registration
  fails because `encoded_packet` is required; the replay output is never created.
- `test_replay_buffer_pause_stop`: all four cases skip after installed libobs
  rejects encoder-info size 192 (supported 184).
- `test_recording_shutdown`: two cases fail after that encoder-info mismatch;
  only the no-recording case passes.

Evidence: `local-test-DQCn0u.log`. Initial scout setup errors (missing private
caption/root include paths, unwrapped implementation headers, and pkg-config's
C-only standard flag reaching C++) were corrected in the scratch harness before
this completed run; they are not claimed as test failures.
The four candidates rejected by the previous slice were not retried or added.
Other XSHM cases register sources, while V4L2 vivid cases need a kernel device
and PipeWire camera cases need a live service; none is claimed exercised here.

## Validation

- RED `1438e2e2c`: focused registration fails on missing XComposite target,
  `local-test-J4W61T.log`. An earlier harness compile error was corrected before
  recording RED.
- Original XComposite CMake scout passes both unchanged cases with GCC
  (`local-test-jIUApZ.log`) and Clang (`local-test-zVMuHK.log`), without skips.
- Combined real Cargo/cc::Build and CMake/CTest replay passes with GCC
  (`local-test-2R6Mln.log`) and Clang (`local-test-tpPFKA.log`): both XComposite
  cases and all eight prior visibility cases through each consumer. Native
  output is forced to cmocka STDOUT and both streams are checked for skips.
- CMake replay evaluates Windows/macOS selections and Linux with renderer off
  (no selected tests), plus owning plugin off (only visibility selected).
  Rust selection covers all three host selections with owner/renderer on/off.
- Focused sorting, portability, graph-slice, cmocka and Rust binary-budget guards
  pass (`local-test-fxGFr5.log`). No absolute inventory totals were added.
- Independent JSON comparison confirms every prior target/test unchanged and
  exactly one new target/test. Existing C tests, public headers, WebSocket,
  workflows and Rust executable inventory are unchanged.
- New registration cost over 25 reduced-debug runs: longest 0.126 s, peak child
  RSS 12,016 KiB, no own Rust threads. Artifact:
  `build_ci_audit/b5-gl2/test-cost.json`. Native XComposite cases use real XCB;
  the combined compilation/CMake replay retains the native-build budget exception.
- Whole-graph legacy replacement passes (`local-test-kOAt1H.log`). Its manual
  fact table initially lacked the new owner fact (`local-test-nQUfQE.log`);
  adding the explicit false default fixed it. Final focused registration also
  passes (`local-test-g7eCq4.log`).
- One primary clud-review found no actionable source/config/documentation findings.
- Range-diff confirms both original visibility commits are unchanged after
  rebasing onto the configure repair: `cb51acd2f` becomes `f3d01713b` and
  `40340baf7` becomes `bf5420e61`.

Logs are retained under `/home/niteris/.clud/tmp/tool-output/local-test-private/`.
Scratch SDK runners and scout trees are under `build_ci_audit/b5-gl2/`.
Installed-library smoke evidence does not establish final-head hybrid versus
C-reference or export parity. Full workspace tests, Windows/macOS execution,
application profiles, device and protocol behavior remain unverified locally.
No PR, hosted CI, attestation, target pruning or soldr GC for this slice.

# Native graph: batch 2 libobs A–F continuation 3

Refs #1390. This slice descends from `test/1390-b2-libobs-af2`, retaining
its parent registrations. It registers two more unchanged headless tests:

- `test_avcapture_audio_planes`: three cases against the production portable
  AVCapture audio-plane helper, retaining the `OBS::libobs` availability guard,
  mac-avcapture include path and real libobs/cmocka linkage. No camera or Apple SDK.
- `test_effect_parser_lifecycle`: eight original-C `ep_free` characterization
  cases, retaining the extracted implementation source, real libobs/caption/cmocka
  linkage and `address,undefined` sanitizer setting. The graphics PCH needs
  caption/Uthash include interfaces and Windows-only w32-pthreads. This exercises
  the C implementation, including when its dependencies use the hybrid core;
  its Rust-static companion remains in the legacy fragment for its owning slice.

Both preserve direct CTest commands and absent timeouts. Existing C tests,
public headers and WebSocket sources are unchanged; no Rust executable is added.

## Local evidence and limits

RED `f9fbe58c2` fails the new Application-category registration check with
`missing portable real-libobs helper` (`local-test-KnX4Sa.log`; Soldr's full
log `20261107T064112Z-home-niteris-dev-obs-rust-obs-studio2.log`). GREEN
registration/parse/selection checks cover five OS families with and without
the libobs availability fact (`local-test-bDSDEm.log`, followed by a rerun
covering the Windows pthread dependency, `local-test-WdDj1m.log`).

GCC and Clang each compile and link both unchanged executables through the
actual Cargo graph and CMake consumer, then pass all 11 cmocka cases against
installed OBS 32.1.2. Cargo `ldd` checks confirm the selected real library.
Cargo replay logs: `local-test-25asH1.log` and `local-test-0vRlvS.log`;
CMake builds: `local-test-Q0WS8d.log` and `local-test-k58Gq2.log`;
CTest: `local-test-vrUbF0.log` and `local-test-inP3y8.log`.
CTest JSON inventories confirm two direct commands and no timeouts.
The final Windows-only dependency does not change either Linux compile.
Fixtures and cost observations are in `/tmp/parity-b2-libobs-af3/`;
logs are in `/home/niteris/.clud/tmp/tool-output/local-test-private/`.

Whole-graph legacy replacement passes (`local-test-cmXHIg.log`), as do scoped
Application/build all-target Clippy (`local-test-HOHWE9.log`, final `local-test-cDDQlX.log`), rustfmt and the
Python graph-slice, sorted graph/list, build-group, shared-file, Rust-static,
Windows diagnostic, sanitizer and include guards. Two guards initially failed
module-style imports; their normal script entry points pass
(`local-test-cxoRPu.log`, `local-test-2Xx4Q5.log`).

Test cost: 25 reduced debug registration observations, longest 0.05 s,
peak RSS 11,544 KiB, no test-owned threads. This is a registration contract;
unchanged native C executables keep their existing execution model.

Final-head hybrid/C-reference runtime and export parity, native Windows/macOS,
and broader app/profile/protocol behavior are not verified here. Full lint and
unittest are not run. Standalone `soldr rustc` hit an unavailable broker daemon
path; a scratch Cargo consumer built successfully through `soldr cargo`.
No target pruning, soldr gc, PR, hosted CI or auto-merge was requested or run.

## Deferred candidates

This is a bounded continuation, not an exhaustive eligibility census. Audio I/O
requires an X display; async audio, deinterlacing and encoder video paths need
core video/graphics. Async-delay pacing belongs to #3829. Parent registrations
already cover the scalar filters and capture retry/format/display helpers.
ALSA wrappers, FFmpeg integrations, file-updater networking and Qt/frontend
cases remain unclaimed here for later slices with their dependency contracts.

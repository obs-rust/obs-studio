# Flatpak packaged runtime qualification (#1760)

The existing `ci.yml` manual entry point accepts `flatpak-runtime-test=true`.
It preserves the full build, AppStream/build-directory/repository validators
and release bundle policy. After validation, the opt-in path transfers the
exported OSTree repository to a fresh Ubuntu 26.04 job. Neither routine PRs nor
routine full builds run this additional qualification by default.

The test installs the actual application and matching Debug extension from that
repository into an isolated XDG data directory. It verifies their commits and
CEF build IDs, and resolves `cef_version_info` source lines through GDB using
the installed Debug extension. It then runs `flatpak run` with a saved profile,
not the builder's staging directory or an empty first launch.

The profile includes PipeWire screen and camera capture, a PulseAudio microphone,
and an actual CEF browser source. The unchanged AppImage testkit supplies a
ScreenCast/Camera portal and microphone tone. Screen/camera producers supply
moving images. The unchanged C++ WebSocket plugin must authenticate over v5,
report the saved sources, start a recording and stop it successfully.

The recording passes the existing AppImage verifier: moving screen and camera,
continuous 440 Hz microphone audio, sufficient frames and clean decoding. An
additional pixel check requires the browser's green HTML rectangle throughout
the decoded recording. The app must load every packaged OBS module, except the
existing hardware-only decklink/NVENC exceptions, and exit cleanly. These are
synthetic devices with software rendering; physical hardware is not covered.

## Cost and evidence

This is additional qualification work. The build logs the uncompressed export
archive size; Actions records upload/download, setup and runtime-test durations.
The test retains installed app/debug commits, module inventory, logs, recording
and elapsed time in the seven-day runtime evidence artifact. The temporary
repository artifact expires after one day. There is no new cache family,
registry publication or external storage.

Count archive/upload and the separate job's setup, download, runtime and cleanup
when reporting qualification runner cost. Keep the routine no-probe build table
separate, and do not count removing this opt-in test as a routine speedup. Runtime
qualification does not establish #1760's warm/cold full-job performance target.

## Local feedback

`uv run --no-project python -m unittest ci.test_flatpak_runtime` checks negative
capture/plugin evidence, absent/insufficient/wrong browser frames, matching Debug refs,
and saved-profile construction. It does not replace the live packaged run.
The local gate cannot attest this hosted opt-in job; ordinary PR CI remains the
merge gate. Keep performance PRs draft until a real qualification run succeeds.

A local FFV1/YUV420p video round trip confirms the browser pixel check accepts
180 green-patch frames and rejects 180 frames without the browser patch. The
first local encoder fixture could not run because the SDK's FFmpeg lacks
libx264; that failure is retained. The FFV1 fixture validates decoding/crop/color
checks, not CEF rendering or the actual packaged H.264 recording.

## Recheck an existing export

For harness-only fixes, `ci.yml` accepts `flatpak-runtime-reuse-run=<run-id>`.
It skips the build-project invocation, downloads the same-repository run's
immutable export, and runs the current harness against it. CI format/data checks
and the final gate still run. The final gate includes this consumer's result.
The read-only Actions token downloads the artifact; no cache is published.

The result records the package source SHA from the producer run, the harness
SHA and producer run ID separately. An old artifact cannot qualify a later
packaging change: build a new export whenever shipped inputs change. The archive
expires after one day; retain the downloaded evidence for the retrospective.
This mode saves a repeated full build, not the fresh runtime session setup.

The first hosted attempt built/exported successfully but stopped before install:
`remote-ls` returned `app/` and `runtime/` kind prefixes, unlike the installed-ref
listing used during local investigation. The exact hosted output now has a
regression. Only valid app/Debug kinds are normalized; branch/architecture and
Debug-extension matching remain strict. No live capture pass is claimed yet.

## Hosted qualification failures retained

The first exported repository qualified in run `39754042722`; its runtime
harness stopped before installation on typed remote refs. Run `39758270223`
reused that immutable export with the corrected parser, installed the app and
Debug extension, confirmed the CEF build ID and resolved `cef_version_info`
source lines with GDB. It then failed to connect to WebSocket: OBS had opened a
default profile. Flatpak resets protected XDG variables after `--env`, so that
option did not select the saved fixture. The harness now uses the installed
runtime's `env` command inside the sandbox to set the fixture config root before
executing the same packaged `/app/bin/obs`. It grants only the existing fixture
directory, and neither changes HOME nor modifies the packaged executable.
See [Flatpak XDG conventions](https://docs.flatpak.org/en/latest/conventions.html).
These failures are retained; no saved capture/browser runtime pass is claimed yet.

Run `39759412764` confirms the saved-profile launch correction: the unchanged
WebSocket plugin loads its saved authenticated configuration, and OBS creates
all four saved sources. The initial GetVersion request arrived before scene
loading completed and received the plugin's explicit `207 NotReady` status.
The harness now waits up to 60 seconds for that read-only readiness check;
other errors and all recording commands fail immediately. Existing failed-run
capture warnings occurred during cleanup and do not prove capture failure.

That package also includes AJA modules. The hosted runner has no AJA hardware,
and the real log explicitly reports both the absent devices and module
initialization failures. The harness records these as unavailable hardware only
when both exact diagnostics are present; a missing library or unexplained
plugin failure still fails. Browser and unchanged WebSocket loading remain
mandatory. This does not claim physical AJA, camera or microphone coverage.

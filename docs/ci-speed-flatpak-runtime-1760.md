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
hardware-only decklink/NVENC exceptions and explicitly diagnosed absent AJA devices, and exit cleanly. These are
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

Run `39760870691` passed readiness, authenticated GetVersion and the exact saved
input inventory; microphone capture opened `obs-test-mic`. The harness then
queried recording status immediately after successful StartRecord, observed
inactive and shut down before the queued recording transition completed.
It now sends StartRecord once and polls only GetRecordStatus for up to 20 seconds.
A never-active recording still fails; no mutation is retried. Full recording,
screen/camera streaming and browser-pixel verification remain outstanding.

Run `39762495293` completes a 12.166-second, 365-frame recording and clean app
shutdown. Screen and camera motion, continuous 440 Hz microphone tone, portal
streaming, plugin inventory and installed CEF debug checks pass. Browser pixels
fail: the decoded frame contains the underlying screen bars where the green
HTML should appear. This failure is retained; module loading is not treated as
proof of browser rendering.

That runner reports llvmpipe and no DRM render node, while the frontend's browser
hardware setting defaults to true. The next fixture explicitly selects the
user-visible `BrowserHWAccel=false` setting in its saved global configuration,
matching the declared software-only environment. The green-pixel threshold
remains unchanged. This is a hypothesis to validate, not a confirmed cause or a
browser pass; hardware browser rendering is outside this synthetic coverage.

## Successful installed-package qualification

[Run 39763463067](https://github.com/obs-rust/obs-studio/actions/runs/39763463067)
passes the complete installed-package scenario with the unchanged export from
producer `39754042722`, package source `8ff154c45`, and harness `eb36fc7b5`.
The app records 361 frames over 12.033 seconds; screen/camera motion and the
continuous 440 Hz tone pass the unchanged recording verifier. All 302 decoded
browser frames after the two-second startup allowance meet the original green
pixel assertion. Authenticated unchanged WebSocket v5, module inventory,
clean shutdown, installed CEF build IDs and GDB source lookup pass.

See [portable runtime evidence and attempt costs](ci-evidence/2026-10-22-flatpak-installed-runtime.json).
Five failed attempts are retained; no cancellation or unchanged retry was used.
This proves software browser rendering on the synthetic runner, not the failed
default hardware setting or physical device/GPU behavior. Upstream missing DWO
warnings remain; source-line lookup is not complete Chromium symbol coverage.
The earlier failed-run entries above are historical evidence, not current holds.

This runtime hold is satisfied for the recorded package. Normal PR CI and the
merge queue still apply. The next full build must qualify the combined cache
restore/reset controls, and the issue's three warm observations, cold measurement
and 50% whole-job target remain unproven. Do not close #1760 on this runtime pass.

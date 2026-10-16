# obs-websocket v5 Tier 4 black-box suite

Black-box protocol tests for the obs-websocket v5 server
(`docs/rust-port/testing-policy.md`, Tier 4). The suite talks to the server
over the wire only — it launches a real headless OBS, connects a WebSocket
client, and asserts on `op`/`d` envelopes, `requestStatus.code` values, event
deliveries and close codes. No implementation hooks, no source access: the
same client runs unchanged against the current C++ server today and against
the future Rust server.

The suite MUST pass against the current C++ server before any
obs-websocket port (Phase 5 item 3).

## Coverage

- Hello/Identify handshake, with and without `authentication` (challenge,
  wrong-password close 4009, request-before-Identify close 4007,
  double-Identify 4008, rpcVersion violations 4003/4010).
- Every request type in `plugins/obs-websocket/docs/generated/protocol.json`
  — 147 request types across all categories (General, Config, Scenes,
  Scene Items, Inputs, Media Inputs, Sources, Filters, Transitions,
  Outputs, Stream, Record, UI, Canvases).
- Event subscription delivery (`SceneCreated`, `CustomEvent`,
  `VendorEvent`), and `EventSubscription::None` isolation.
- `GetStats.gpuAcceleration` with OpenGL forced onto llvmpipe: not green,
  and the startup log warns (obs-rust/obs-studio#846).
- Linux, in the same run: exactly one libsrt mapped into the OBS process.
  With two builds of libsrt in one process (the SRT output's and FFmpeg's)
  OBS aborts on quit after any SRT use.
- Request batches: serial, `haltOnFailure`, no-halt error propagation, and
  parallel execution acceptance.
- Error codes for malformed JSON (close 4002), unknown opcode (4006),
  missing `requestType` (4003), missing/invalid fields (300/401), unknown
  request types (204) and not-found resources (600).

## Requirements

- Linux with `xvfb` (the suite runs `xvfb-run -a obs ...`; without
  `xvfb-run` it runs `obs` on the existing `$DISPLAY`, e.g. a manually
  started `Xvfb :97` with `DISPLAY=:97`). `WAYLAND_DISPLAY` is dropped and
  Qt is forced to `xcb`, so OBS never opens on the developer's session.
- An OBS binary containing obs-websocket 5.x — the in-repo build output or
  a packaged OBS >= 30 (websocket >= 5.4).
- Python >= 3.10 and `websockets==13.1` (pinned; e.g.
  `uvx --with websockets==13.1 python test/websocket-tier4/run_tier4.py`).

## Run

```sh
uv run test/websocket-tier4/run_tier4.py                # uses `obs` on PATH
uv run test/websocket-tier4/run_tier4.py --obs /path/to/obs
TIER4_PORT=4460 uv run test/websocket-tier4/run_tier4.py
TIER4_VERBOSE=1 uv run test/websocket-tier4/run_tier4.py  # log each status code
uv run test/websocket-tier4/run_tier4.py --only gpu     # GPU status check (#940) and the libsrt check only
```

Flags:

- `--no-spawn` — attach to an already-running server at `TIER4_PORT`
  (skips headless launch; implies `--skip-auth-phase`).
- `--skip-auth-phase` — only run the unauthenticated half.

The server under test listens on `TIER4_PORT`; the client only ever
connects to `127.0.0.1`. The stream service is pointed at an empty RTMP
server, so `StartStream` fails locally and never sends anything off-host.

Not wired into CI or the local gate: it needs a full OBS build and an X
server. Verified locally against OBS 32.1.2 (Xvfb): 179/179 checks,
147/147 request types.

Exit status is 0 only when every check passes and all 147 documented
request types were exercised. A full run takes ~4 minutes: the matrix
deliberately includes output actuators (Start/Stop stream, output,
virtualcam, replay buffer) which need real state transitions, and a
second server instance is started with `auth_required: true` for the
password handshake.

The suite seeds its own resources (scenes, a color source, a media input,
a filter, a record directory) via the wire before asserting against them,
and restores nothing — it owns the isolated `$HOME` profile it launches
OBS under.

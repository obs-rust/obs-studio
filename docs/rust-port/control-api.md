# Control API

Extend obs-mcp; preserve existing tool names, successful text/image payloads,
headless stdio and the unchanged C++ obs-websocket plugin. New tools use
`domain_verb`: `vcam_start`, `vcam_stop`, `vcam_status`, `vmic_status`,
`media_restart`, `media_status`, `stats_get`, `stats_reset`, `wait_for`,
`pipeline_dump`.

## Contract

TypedTool declares serde/schemars input/output structs, NAME and ANNOTATIONS.
Inputs reject unknown fields and default optional fields. The Tool adapter
publishes inputSchema/outputSchema and returns structuredContent alongside JSON
text (and screenshot image content). The committed schema snapshot must match
schemas generated from the registered types. Backend additions use request and
response structs, never positional multi-argument methods.

Malformed requests and typed arguments are JSON-RPC errors. Domain failures
return isError and ToolError {code, category, message, retryable}, using
obs-websocket v5 codes: 600 ResourceNotFound, 207 NotReady, 604 InvalidResourceState
and 702 RequestProcessingFailed. Categories are independent of numeric ranges.
Read operations have readOnlyHint; start/stop are idempotent.

## State and measurements

`wait_for {condition, timeout_ms}` first checks, then waits within the deadline;
it returns {satisfied, elapsed_ms, last_value}. Condition is a typed enum:
OutputState {output,state}, SourceActive {name}, MediaState {name,state},
StatAtMost {metric,value}. Start accepts optional wait for bounded auto-wait.
Media uses obs_source_media_*; vcam uses virtualcam_output and its private
proc_handler. stats_get reports per-stage render/vcam/vmic {live,
latency_min_ns, latency_max_ns, frames_total, frames_dropped, xruns}, render
counters and {av_offset_ms, measured_from}. Unmeasured values remain unavailable;
queue estimates never masquerade as end-to-end measurements. stats_reset defines
a new counter baseline. pipeline_dump reports the current graph.

## Attachment

A loadable in-app module serves the same newline-delimited JSON-RPC Server on
$XDG_RUNTIME_DIR/obs-studio-rust/control.sock (0600). It uses the running OBS
instance; headless stdio still owns its own instance. `--mcp-attach [socket]`
proxies stdio to that socket. Socket ownership and cleanup follow module
lifecycle. OS code stays in src/platform or src/ffi. Windows named pipes require
a follow-up; unsupported attachment returns a clear error.

## Delivery

Parent #4444 has ordered native sub-issues: #4445 typed tools/errors/schema,
#4446 device/media/stats, #4447 waiting, #4448 socket/module/proxy, #4449 fluent
Rust client and ci/vcam_av_sync.py MCP transport. Each slice targets master
from a fresh infra/control-api-* branch with RED then GREEN shared MCP tests.
The client reuses server types; Python retains websocket and adds --transport
mcp plus fallback. Real GUI/device validation runs only in bosn containers.

## Attach to the application

Build `soldr cargo build -p obs-mcp-module` and install the resulting
`libobs_mcp_module.so` in OBS's normal module search path. The module binds
`$XDG_RUNTIME_DIR/obs-studio-rust/control.sock` with mode 0600 in a private
directory. It shares the app's libobs and stops its worker on frontend EXIT,
prior to libobs shutdown. It never starts a second OBS instance.

`obs-studio-rust --mcp-attach [socket]` relays stdio to that instance. The same
newline JSON-RPC Server handles both transports. At most 32 clients share the
control thread; requests are serial, bounded to 16 MiB per buffered frame.
`wait_for` blocks the control thread, while OBS render/audio threads continue.
Windows named pipes are unsupported pending #4455.

For headless CI use `obs-studio-rust --mcp-listen [socket]` (native build).
Keep its stdin open for the test's lifetime; EOF stops the socket and libobs.
The attached backend resolves frontend scenes and output names on each call,
preserves coordinate settings, and queues scene changes through the graphics
thread so the frontend's UI mutation path cannot block shutdown.

## Clients and the A/V probe

```rust,ignore
let obs = obs_mcp::client::Client::attach(Default::default())?;
obs.vcam().start(obs_mcp::tools::control::StartInput {
    wait: true, ..Default::default()
})?;
obs.wait_for(condition).timeout_ms(5000).call()?;
```

`uv run --script ci/vcam_av_sync.py measure --transport mcp --mcp-socket PATH`
creates a uniquely named media probe in the program scene, waits for playback,
starts independent ffmpeg v4l2/PulseAudio capture, restarts the clip and reports
A/V offset, per-path latency and `stats_get`. `source_remove` cleans up the probe.
`--transport auto` (default) falls back to MCP only when the websocket connection
cannot be opened. `--transport websocket` preserves the original path. Auth and
protocol failures are surfaced rather than retried with a second transport.
Capture timing is relative to the media restart response wall clock; the server
keeps end-to-end offset null until an independent capture probe measures it.
Run actual device capture only inside bosn. No GUI interaction is required.

# In-process MCP server

`obs-studio-rust --mcp` starts libobs inside the process and serves the
[Model Context Protocol](https://modelcontextprotocol.io) over stdin/stdout, so
an agent can create scenes and sources, start and stop outputs and take
screenshots. The server (`rust/obs-mcp`) calls libobs directly; it does not use
obs-websocket (epic #3283).

## Build it first

The binary links libobs only with the `bundled-native` feature, which compiles
the C libobs through `obs-sys` (the same toolchain as the other native
contracts; see [cc-build.md](cc-build.md)):

Build the release `libobs-rust` archive first and point `obs-sys` at it (the
native build needs the dependencies listed in [cc-build.md](cc-build.md)):

```sh
soldr cargo build -p libobs-rust --release
OBS_BUILD_RUST_ARCHIVE="$PWD/target/release/liblibobs_rust.a" \
  soldr cargo build -p obs-studio-rust --features bundled-native
```

Without the feature, `--mcp` exits with an error that says so.

## Run it

```sh
target/debug/obs-studio-rust --mcp
```

- stdin and stdout carry newline-delimited JSON-RPC and nothing else. libobs
  logs and all diagnostics go to stderr (the process points file descriptor 1
  at stderr before libobs starts and keeps the original stdout for the
  protocol).
- Closing stdin shuts the server and libobs down and exits with status 0.
- The server runs headless: libobs starts without a video subsystem, so
  scene, source and statistics tools work, while `screenshot` and live
  outputs report an error until a graphics module is wired in.
- The record and stream tools control the libobs outputs named
  `simple_file_output` and `simple_stream`; they report "not configured" until
  something creates those outputs.

## Claude Code and clud

The repo root has a [`.mcp.json`](../../.mcp.json) that registers a server
named `obs` running `target/debug/obs-studio-rust --mcp`. Build the binary as
above, then start Claude Code or clud in the repository root and approve the
`obs` project server; the tools appear as `mcp__obs__scenes_list`,
`mcp__obs__scene_create`, and so on.

## Tests

Tier 4 (`obs-tests-app`, module `mcp_stdio`, feature `bundled-native`) spawns
the built binary with `--mcp` and drives it over stdio: `initialize`,
`notifications/initialized`, `tools/list`, `scene_create`, `scenes_list`,
`resources/read obs://scenes`, then closes stdin and checks the clean exit.

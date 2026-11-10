# MCP control server

From the repository root, build the libobs-enabled application with:

```sh
./mcp --build
```

The command prints the executable path, `target/mcp/debug/obs-studio-rust`.
It first builds the release `libobs-rust` archive, then builds the application
with `bundled-native`, passing the archive automatically. This separate target
directory prevents an ordinary featureless application build from overwriting
the MCP application. Install soldr as described in [CI tooling](../ci-tooling.md).

On NixOS the launcher automatically enters the declared `ci/mcp-shell.nix`
environment. This requires nix-shell and an available nixpkgs channel. The shell
provides the compiler, FFmpeg, X11 development metadata and other native SDKs;
no X server is required. Other hosts must have the native libobs development
dependencies installed. Nix may fetch dependencies on the first invocation.
`OBS_MCP_TARGET_DIR` optionally selects another build directory.

`.mcp.json` runs `./mcp --mcp`. The launcher builds incrementally before starting
the server; build diagnostics go to stderr and protocol traffic stays on stdout.
With no arguments `./mcp` also starts the stdio server. For example:

```sh
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | ./mcp --mcp
```

The same launcher accepts socket arguments:

```sh
./mcp --mcp-listen /run/user/1000/obs-control.sock
./mcp --mcp-attach /run/user/1000/obs-control.sock
```

Keep the listening process's stdin open. The socket uses mode 0600; its parent
directory must already exist. Attach proxies newline-delimited JSON-RPC over
stdio. See [the control API](control-api.md) for tools, schemas and metrics.
GUI/device validation belongs in bosn containers; these headless examples do
not open a GUI or require a camera device.

Run bootstrap regressions with:

```sh
uv run --no-project python -m unittest ci.test_mcp_build
```

The stdio host starts libobs without a video subsystem. Screenshot and live
outputs report unavailable until a graphics module is configured. Record and
stream tools also require their corresponding outputs to be configured.
Closing stdin shuts down the server cleanly. Existing Tier 4 coverage remains
in `obs-tests-app` (`mcp_stdio`, feature `bundled-native`).

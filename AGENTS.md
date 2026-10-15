# Agent instructions

This repository is governed by the [Constitution of the obs-rust Project](https://github.com/obs-rust/constitution/blob/main/CONSTITUTION.md), which supersedes every other document here, this one included.

This repository is being converted from C/C++ to Rust while keeping the public
API (libobs C API/ABI, obs-websocket v5 protocol, frontend and scripting APIs)
compatible.

## Mandatory reading

Before porting any code, writing any test, or touching `rust/`, `Cargo.toml`,
or `test/`, read and follow
[docs/rust-port/testing-policy.md](docs/rust-port/testing-policy.md) exactly.
It defines the four required test tiers (Rust unit tests, unchanged C tests
against the Rust implementation, differential tests against the original C,
and black-box protocol tests) and the per-unit workflow.

Before picking up or filing an issue, or opening a PR, read
[docs/rust-port/coordination.md](docs/rust-port/coordination.md): claim
before coding, search open PRs first, target master, RED then GREEN commits.

Before building, testing or pushing, read
[docs/ci-tooling.md](docs/ci-tooling.md): soldr, setup-soldr, bosn, act2,
zackees/ci.yml, the local gate and CI attestation, and how to use them so
many agents don't overload the shared machines and CI runners.

## Merge gate: PR CI

PR CI on `obs-rust/obs-studio` is the only check that decides a merge. Open a
PR (a draft is fine) as soon as the RED test exists; a red CI run on a PR is
expected, not a process failure. Do not build fork or self-hosted CI to
pre-validate PRs. What stays mandatory: RED then GREEN, differential and Tier 2
coverage, and no edits to C tests or public headers.

## Local gate (recommended, not required)

The local gate gives faster feedback and, when stamped, lets CI skip the Linux
unit-test jobs. Without a stamp, PR CI runs the full suite. Environment
errors (soldr permissions, disk, Docker, host FFmpeg): see
[docs/rust-port/troubleshooting.md](docs/rust-port/troubleshooting.md).

- `./lint` (8 s): clang-format 22.1.3 and gersemi 0.25.0 pinned via uvx, rustfmt, clippy, and the repo Dylints (`dylints/*`). `./lint --fix` rewrites files.
- `./unittest`: Rust Tiers 1-3, then Tier 2 (cmocka with `ENABLE_RUST_LIBOBS` OFF/ON in Docker; ~30-40 s warm thanks to ccache).
- To let CI skip the Linux unit-test jobs, commit, then run
  `uvx --from git+https://github.com/zackees/ci.yml@1a970f01574447d681571ff3d0b8de56cb983dcc ci-lint local-gate run`.
  It runs every lane in `local-gate.toml` (unchanged lanes are cached) and stamps HEAD with attestation trailers. Push the stamped commit as-is; any later change invalidates the stamp.
- PRs that only touch `rust/`, `test/cmocka/`, the local gate or docs skip the full OBS builds (quick gate in `build-project.yaml`). Add the `ci-full` label to force them.

## Quick rules

- Use `soldr cargo ...`, never bare `cargo`. `.claude/settings.json` wires `ci/hooks/tool_guard.py`, a PreToolUse hook that denies bare Rust tools and bare python/pip (clud sessions get the same rule from clud; `.clud/settings.json` turns on its soldr shims). It denies, after parsing the command: bare `cargo`, `cargo-clippy`, `cargo-fmt`, `clippy-driver`, `rustc`, `rustdoc`, `rustfmt`, `rustup`, `rust-gdb`, `rust-lldb`, `rust-analyzer`, `python`, `python3`, `pip`, `pip3`, also behind `FOO=bar`, `env`, `exec`, `sudo`, `time` or `bash -c`; text inside quoted arguments and heredoc bodies is never matched. Codex has no repo hook: follow the rule by hand. `ci/test_tool_guard.py` (part of `./lint`) also fails on bare Rust tools in `ci/` and `rust/tools/` shell scripts.
- Run Python through `uv run` (or `uvx`), never bare `python`/`python3`; standalone scripts use `#!/usr/bin/env -S uv run --script`. `ci/test_uv_run.py` (part of `./lint`) enforces it; see the testing policy's "Python runs through uv" rule.
- Pin `rust-toolchain.toml` to the latest Rust the CI soldr version supports natively, never beyond; bump it only after a soldr bump (see the testing policy).
- Never edit existing C tests or public C headers to make a port pass.
- Name crates, binaries and tags per
  [docs/rust-port/naming.md](docs/rust-port/naming.md) (`ci/test_naming.py`
  checks crates): `obs-<area>` ports, product slug `obs-studio-rust`.
- Host-specific code (`#[cfg(windows|unix|target_os..)]`, `cfg!()`, `std::os::*`,
  `libc::`, Win32) goes only in the crate's `src/platform/` (single
  `cfg_select!` in `platform/mod.rs`) or `src/ffi/`. A pre-expansion Dylint
  (`dylints/ban_platform_cfg_outside_boundary`) fails `./lint` otherwise; see
  the testing policy's "Platform boundary" rule.
- Start every task by fetching and rebasing onto `origin/master`
  (`git fetch origin && git rebase --autostash origin/master`), so you build,
  test and diff against current master. See
  [coordination.md](docs/rust-port/coordination.md).
- Every port starts RED (failing test against stubs) and ends GREEN.
- Do not add new GitHub Actions workflow files without explicit sign-off.
- Packaging changes (`build-aux/appimage/`, the Flatpak manifest, anything that
  decides what ships in a release artifact) must be covered by a test that runs
  the packaged artifact the way a user does, including a saved profile with the
  sources users depend on (screen capture, camera, microphone), not only a first
  start with an empty profile. `test-appimage.sh` is that test for the AppImage;
  extend it when a change adds something loaded at runtime, and show it failing
  without the change (obs-rust/obs-studio#426).

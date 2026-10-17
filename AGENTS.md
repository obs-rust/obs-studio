# Agent instructions

This repository is governed by the [Constitution of the obs-rust Project](https://github.com/obs-rust/constitution/blob/main/CONSTITUTION.md), which supersedes every other document here, this one included.

This repository is being converted from C/C++ to Rust while keeping the public
API (libobs C API/ABI, obs-websocket v5 protocol, frontend and scripting APIs)
compatible.

## Worker role

When you take an issue to work on (claim, port, fix, test, or build, lint,
push and open a PR), you are a worker: invoke the `obs-worker` skill
([.claude/skills/obs-worker/SKILL.md](.claude/skills/obs-worker/SKILL.md)).
Agents without skills read that file by hand. It walks the documents below
in order, from rebase to merge queue.

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
A PR with a user-visible change adds its line to
[docs/features.md](docs/features.md) (the features ledger).

Before adding, renaming or building C/C++/Objective-C sources, or touching
`cmake/common/`, `cmake/pch/` or a `build/` group, read [BUILD.md](BUILD.md).

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
- `./unittest`: Rust Tiers 1-3, then Tier 2 (cmocka against the C libobs and libobs-with-rust, one build in Docker; ~30-40 s warm thanks to zccache).
- To let CI skip the Linux unit-test jobs, commit, then run
  `uvx --from git+https://github.com/zackees/ci.yml@1a970f01574447d681571ff3d0b8de56cb983dcc ci-lint local-gate run`.
  It runs every lane in `local-gate.toml` (unchanged lanes are cached) and stamps HEAD with attestation trailers. Push the stamped commit as-is; any later change invalidates the stamp.
- A PR runs only the lanes its changed paths select (the `LANES` table in `ci/ci_scope.py`: Fast Gate always, Tier 2 for libobs/linked crates/`test/cmocka/`, Qt for `frontend/`, a Linux build of only the touched `plugins/<x>`/`shared/<x>` targets; macOS/Windows only for their own platform files). The full OBS app builds never run from a PR path. No label changes what runs. A master push adds every lane a PR defers (Tier 2, Qt, the workspace tests) but builds macOS, Windows and Flatpak only for paths that select them; schedule, workflow_dispatch, tags and the release gate run everything (#436).

## Quick rules

- Use `soldr cargo ...`, never bare `cargo`. `.claude/settings.json` wires `ci/hooks/tool_guard.py`, a PreToolUse hook that denies bare Rust tools and bare python/pip (clud sessions get the same rule from clud; `.clud/settings.json` turns on its soldr shims). It denies, after parsing the command: bare `cargo`, `cargo-clippy`, `cargo-fmt`, `clippy-driver`, `rustc`, `rustdoc`, `rustfmt`, `rustup`, `rust-gdb`, `rust-lldb`, `rust-analyzer`, `python`, `python3`, `pip`, `pip3`, also behind `FOO=bar`, `env`, `exec`, `sudo`, `time` or `bash -c`; text inside quoted arguments and heredoc bodies is never matched. Codex has no repo hook: follow the rule by hand. `ci/test_tool_guard.py` (part of `./lint`) also fails on bare Rust tools in `ci/` and `rust/tools/` shell scripts.
- Run Python through `uv run` (or `uvx`), never bare `python`/`python3`; standalone scripts use `#!/usr/bin/env -S uv run --script`. `ci/test_uv_run.py` (part of `./lint`) enforces it; see the testing policy's "Python runs through uv" rule.
- Pin `rust-toolchain.toml` to the latest Rust the CI soldr version supports natively, never beyond; bump it only after a soldr bump (see the testing policy).
- Never edit existing C tests or public C headers to make a port pass.
- Name crates, binaries and tags per
  [docs/rust-port/naming.md](docs/rust-port/naming.md) (`ci/test_naming.py`
  checks crates): `obs-<area>` ports, product slug `obs-studio-rust`.
- Don't move upstream C/C++ files to restructure: the upstream tree
  stays in place, Rust goes in the `rust/` workspace, and a fully ported C
  component is deleted, not moved. See the layout section of
  [docs/rust-port/naming.md](docs/rust-port/naming.md) (#1051). The one
  rename allowed is in place, to the build-role names of
  [BUILD.md](BUILD.md) (`foo.c` to `foo.c.h`).
- Native C/C++/Objective-C builds follow [BUILD.md](BUILD.md): explicit unity
  groups in each component's `build/`, one precompiled header per group
  that is each implementation file's only include, unity and PCH always on
  (`ci/test_build_groups.py`).
- C/C++ host code stays in OS-named files (`obs-nix.c`, `platform-windows.cpp`,
  `win-capture/`, ...). Upstream files keep their names; new host files use
  `hosts/<os>/` folders or `*-host-<os>.c` files, `<os>` one of `windows`,
  `macos`, `linux`, `posix` (#1009). `ci/platform_ifdef_ratchet.py` (in `./lint`) fails a
  new host conditional or OS header in a shared file, and a platform file that
  names another OS. Shared libobs core calls host code through one interface
  per capability in `libobs/obs-host/<capability>.h` (`OBS_HOST_EXTERN`); a
  host implements it in its OS file or builds `obs-host/noop/<capability>.c`,
  picked in `libobs/cmake/os-<os>.cmake`.
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

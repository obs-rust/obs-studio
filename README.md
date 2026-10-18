# obs-studio-rust

> [!IMPORTANT]
> ## **CI is optimized: full Rust, with C compilation**
> The refactor ([#930](https://github.com/obs-rust/obs-studio/issues/930)) is done. CI has been
> optimized and converted to full Rust with C compilation, so runs are fast again.
>
> **Contributors: please prioritize converting core OBS C/C++ code to Rust.** Every core module
> moved shrinks the remaining port; once the core is ported we drop the parity efforts — the
> C/Rust parity and differential checks go away with the C they compare.

[![Contributor leaderboard workflow status](https://github.com/obs-rust/obs-studio/actions/workflows/leaderboard.yml/badge.svg?branch=master)](https://github.com/obs-rust/obs-studio/actions/workflows/leaderboard.yml)
[![Merge queue for master](docs/images/merge-queue.svg)](https://github.com/obs-rust/obs-studio/queue/master)
[![Discussions: got an idea?](docs/images/discussions.svg)](https://github.com/obs-rust/obs-studio/discussions)
[![Reddit: r/ObsRust](https://img.shields.io/badge/Reddit-r%2FObsRust-FF4500?logo=reddit&logoColor=white)](https://www.reddit.com/r/ObsRust/)
[![OBS Rust Discord Server](https://img.shields.io/badge/Discord-OBS%20Rust-5865F2?logo=discord&logoColor=white)](https://discord.gg/6MWWX4x9pf)

<p align="center"><img src="docs/images/obs-rust-logo.png" alt="obs-rust logo" width="128"></p>

This is an independent rewrite of the obsproject's obs-studio.

Same obs studio, but without the bugs. AI generated issues and PRs welcome.

Got an idea? [Start a conversation](https://github.com/obs-rust/obs-studio/discussions).

# Features

What OBS-Studio-Rust does differently from upstream OBS Studio, for a user:
new features, changed behavior and fixed upstream bugs, by area, with links
to the issues and pull requests. See the
[features ledger](docs/features.md).

# Fixing bugs with AI + Rust

https://github.com/user-attachments/assets/4d9af616-bd76-468e-93ac-ef8ef44a0efb




# Launch speed

4.3× faster to the main window and 19% less memory on an NVIDIA streaming
rig (#1168, #1170, #1171, #1172).

<p align="center"><img src="docs/images/launch-speed.png" alt="OBS-Rust launch speed: 4.09 s to 0.95 s, 307 to 250 MiB" width="600"></p>

# Contributors leaderboard

Ranked by merged PRs only, credited to the original author even when a
maintainer re-landed the work (counted once). Updated every 4 hours.

[![obs-rust contributors leaderboard](https://raw.githubusercontent.com/obs-rust/obs-studio/leaderboard/leaderboard.jpg)](https://obs-rust.github.io/obs-studio/)

[Open the live leaderboard](https://obs-rust.github.io/obs-studio/)


# How CI works here

Lots of people and AI agents work on this repo at the same time, so CI is
built to do as little repeated work as possible:

- **Every pull request gets a quick check.** It runs the Rust and C tests on
  Linux, plus macOS or Windows only when your change touches those
  platforms. It does not build every app package.
- **The full build runs before each release.** Once a day, and before
  anything is published, CI builds and tests everything: every platform,
  the full app, Flatpak and the release packages. A beta only ships if
  all of it passes.
- **You can prove your change locally first.** One command (`ci-lint
  local-gate run`) runs the same Linux checks on your machine and signs
  your commit. CI sees the signature and skips the checks you already
  ran.
- **Builds are cached and shared.** Rust builds go through `soldr`, which
  reuses compiled code across branches and machines.

The full guide, with the tools, rules and commands, is in
[docs/ci-tooling.md](docs/ci-tooling.md). If you're contributing, please
read it before you push: it explains how to avoid overloading the
shared machines and CI.

Want to see the original README from OBS Project? [Click here](README.upstream.md).

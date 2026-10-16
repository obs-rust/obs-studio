# obs-studio-rust

> [!WARNING]
> ## **CI REFACTORING IN PROGRESS**
> **You may see extremely long CI run times for the next six hours (until about 2026-10-16 10:00 UTC).**
> **It will be much, much, much faster when this is done.** Progress: [#930](https://github.com/obs-rust/obs-studio/issues/930).

[![Contributor leaderboard workflow status](https://github.com/obs-rust/obs-studio/actions/workflows/leaderboard.yml/badge.svg?branch=master)](https://github.com/obs-rust/obs-studio/actions/workflows/leaderboard.yml)
[![Merge queue for master](docs/images/merge-queue.svg)](https://github.com/obs-rust/obs-studio/queue/master)
[![Discussions: got an idea?](docs/images/discussions.svg)](https://github.com/obs-rust/obs-studio/discussions)

<p align="center"><img src="docs/images/obs-rust-logo.png" alt="obs-rust logo" width="128"></p>

This is an independent rewrite of the obsproject's obs-studio.

Same obs studio, but without the bugs. AI generated issues and PRs welcome.

Got an idea? [Start a conversation](https://github.com/obs-rust/obs-studio/discussions).

# Fixing bugs with AI + Rust

https://github.com/user-attachments/assets/4d9af616-bd76-468e-93ac-ef8ef44a0efb




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


# Upstream/Original

## OBS Studio &lt;<https://obsproject.com>&gt;

[![OBS Studio Build Status - GitHub Actions](https://github.com/obsproject/obs-studio/actions/workflows/push.yaml/badge.svg?branch=master)](https://github.com/obsproject/obs-studio/actions/workflows/push.yaml?query=branch%3Amaster)

[![OBS Studio Translation Project Progress](https://badges.crowdin.net/obs-studio/localized.svg)](https://crowdin.com/project/obs-studio)

[![OBS Rust Discord Server](https://img.shields.io/badge/Discord-OBS%20Rust-5865F2?logo=discord&logoColor=white)](https://discord.gg/6MWWX4x9pf)

[![Upstream OBS Studio Discord Server](https://img.shields.io/discord/348973006581923840.svg?label=Upstream%20OBS%20Discord&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://obsproject.com/discord)

# What is OBS Studio?

OBS Studio is software designed for capturing, compositing, encoding,
recording, and streaming video content, efficiently.

It's distributed under the GNU General Public License v2 (or any later
version) - see the accompanying COPYING file for more details.

# Quick Links

- Website: https://obsproject.com

- Help/Documentation/Guides: https://github.com/obsproject/obs-studio/wiki

- Forums: https://obsproject.com/forum/

- Build Instructions: https://github.com/obsproject/obs-studio/wiki/Install-Instructions

- Packaging for a distribution (which version goes where): [build-aux/README.md](build-aux/README.md#packaging-version-numbers)

- Developer/API Documentation: https://obsproject.com/docs

- Donating/backing/sponsoring: https://obsproject.com/contribute

- Bug Tracker: https://github.com/obsproject/obs-studio/issues

# Contributing

- If you would like to help fund or sponsor the project, you can do so
  via [Patreon](https://www.patreon.com/obsproject), [OpenCollective](https://opencollective.com/obsproject), or [PayPal](https://www.paypal.me/obsproject).  See our [contribute page](https://obsproject.com/contribute) for more information.

- If you wish to contribute code to the project, please make sure to
  read the coding and commit guidelines:
  https://github.com/obs-rust/obs-studio/blob/master/CONTRIBUTING.md

- Code for the project follows the code style guidelines, located
  here: https://github.com/obs-rust/obs-studio/blob/master/CODESTYLE.md

- Before building, testing or pushing (people and AI agents alike), read the
  CI and build tooling guide, which explains how to avoid overloading the
  shared machines and CI:
  [docs/ci-tooling.md](docs/ci-tooling.md)

- Developer/API documentation can be found here:
  https://obsproject.com/docs

- If you wish to contribute translations, do not submit pull requests.
  Instead, please use Crowdin.  For more information read this page:
  https://obsproject.com/wiki/How-To-Contribute-Translations-For-OBS

- This project and every repository in the obs-rust organization are
  governed by the Constitution of the obs-rust Project, which supersedes
  all other project documents:
  https://github.com/obs-rust/constitution/blob/main/CONSTITUTION.md

- Other ways to contribute are by helping people out with support on
  our forums or in our community chat.  Please limit support to topics
  you fully understand -- bad advice is worse than no advice.  When it
  comes to something that you don't fully know or understand, please
  defer to the official help or official channels.

# SAST Tools

[PVS-Studio](https://pvs-studio.com/pvs-studio/?utm_source=website&utm_medium=github&utm_campaign=open_source) - static analyzer for C, C++, C#, and Java code.

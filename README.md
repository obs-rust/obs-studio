# obs-studio-rust

> [!IMPORTANT]
> **We’re unifying OBS into one hybrid Rust/C/C++ build.**
> Prioritize migrating code to Rust and retiring the C/C++ implementations it replaces.
> Each completed port becomes part of the application, with compatibility and parity
> checks supporting the transition.

[![OBS Rust Discord Server](https://img.shields.io/badge/Discord-OBS%20Rust-5865F2?logo=discord&logoColor=white)](https://discord.gg/6MWWX4x9pf)
[![Reddit: r/ObsRust](https://img.shields.io/badge/Reddit-r%2FObsRust-FF4500?logo=reddit&logoColor=white)](https://www.reddit.com/r/ObsRust/)
[![Discussions: got an idea?](docs/images/discussions.svg)](https://github.com/obs-rust/obs-studio/discussions)
[![Contributor leaderboard workflow status](https://github.com/obs-rust/obs-studio/actions/workflows/leaderboard.yml/badge.svg?branch=master)](https://github.com/obs-rust/obs-studio/actions/workflows/leaderboard.yml)
[![Merge queue for master](docs/images/merge-queue.svg)](https://github.com/obs-rust/obs-studio/queue/master)

<p align="center"><img src="docs/images/obs-rust-logo.png" alt="obs-rust logo" width="128"></p>

This is an independent rewrite of the obsproject's obs-studio.

Same obs studio, but without the bugs. AI generated issues and PRs welcome.

Got an idea? [Start a conversation](https://github.com/obs-rust/obs-studio/discussions).

Want to help? Run this:

```
$PROMPT="/loop https://raw.githubusercontent.com/obs-rust/obs-studio/refs/heads/master/.claude/skills/obs-worker/SKILL.md"
claude --dangeriously-skip-permissions $PROMPT
```

**Want to help port OBS to Rust?** Paste this into your AI agent:

```text
You are a worker in obs-rust/obs-studio, the Rust rewrite of OBS Studio.
Take exactly one issue from claim to an open pull request, then stop.
1. Clone https://github.com/obs-rust/obs-studio if you are not in it.
   Read AGENTS.md, then .claude/skills/obs-worker/SKILL.md, and follow it.
   It sends you to docs/rust-port/coordination.md and
   docs/rust-port/testing-policy.md: read both before you code.
2. Pick one open issue from label:ready (fall back to
   label:needs-investigation). Skip any with a recent "Claiming:" comment
   or an open PR that names it. Comment "Claiming: #N" with a short plan.
3. Branch from master. Commit a failing test first (RED), then the fix
   that makes it pass (GREEN). Run ./lint. One PR per issue.
4. Open the PR against master, describing RED and GREEN.
5. Do not merge, never push to master, and never commit secrets.
   Report the PR URL and stop. Maintainers queue PRs; you don't.
Stuck? Ask in the Discord linked in the README, or in Discussions.
```

# Conversion progress

How much of **libobs** is Rust — the part that has to be ported. The rest of
the app can stay C++ in the hybrid build. Measured by
[tools/conversion/measure.py](tools/conversion/measure.py); click the card
for the full breakdown.

[![libobs port progress, click for the full breakdown](https://raw.githubusercontent.com/obs-rust/obs-studio/leaderboard/conversion-minimal.jpg)](https://obs-rust.github.io/obs-studio/conversion.html)

> [!TIP]
> **Come talk to us on [Discord](https://discord.gg/6MWWX4x9pf).** We're a
> new project, and the people building it are there. Ask a question, tell us
> what works or breaks on your setup (AppImage, .deb, Flatpak, your plug-ins and
> capture devices), suggest an idea, or follow the port as it lands. No
> question is too small.

# Features

What OBS-Studio-Rust does differently from upstream OBS Studio, for a user:
new features, changed behavior and fixed upstream bugs, by area, with links
to the issues and pull requests. See the
[features ledger](docs/features.md).

# Fixing bugs with AI + Rust

Want to fix one yourself? Contributors, human or AI-assisted, chat about
ports and pull requests on [Discord](https://discord.gg/6MWWX4x9pf).

https://github.com/user-attachments/assets/4d9af616-bd76-468e-93ac-ef8ef44a0efb




# Launch speed

4.3× faster to the main window and 19% less memory on an NVIDIA streaming
rig (#1168, #1170, #1171, #1172).

<p align="center"><img src="docs/images/launch-speed.png" alt="OBS-Rust launch speed: 4.09 s to 0.95 s, 307 to 250 MiB" width="600"></p>

# Contributors leaderboard

The top five, ranked by merged PRs only, credited to the original author even when a
maintainer re-landed the work (counted once). Updated every 4 hours.

[![obs-rust contributors leaderboard](https://raw.githubusercontent.com/obs-rust/obs-studio/leaderboard/leaderboard.jpg)](https://obs-rust.github.io/obs-studio/)

[See all contributors](https://obs-rust.github.io/obs-studio/#all)


# How CI works here

Lots of people and AI agents work on this repo at the same time, so CI is
built to do as little repeated work as possible:

- **Every pull request gets a quick check.** It runs the Rust and C tests on
  Linux, plus macOS or Windows only when your change touches those
  platforms. It does not build every app package.
- **The full build runs before each release.** Once a day, and before
  anything is published, CI builds and tests everything: every platform,
  the full app, a Flatpak build check and the release packages. A beta
  only ships if all of it passes.
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

# Q&A

**Where can I get help, or report something that isn't quite a bug?**

Join us on [Discord](https://discord.gg/6MWWX4x9pf): it's the quickest way to
reach the people working on OBS Rust. For a reproducible bug, please also
[file an issue](https://github.com/obs-rust/obs-studio/issues/new/choose); for
longer ideas, [start a discussion](https://github.com/obs-rust/obs-studio/discussions).

**Does OBS Rust support all of the plug-ins?**

Yes. OBS Rust is fully backwards compatible with OBS Studio — the
plug-ins you already have should work as-is. If you find one that
doesn't, that's a bug: please [file an issue](https://github.com/obs-rust/obs-studio/issues/new/choose)
and link it here so we can track it.

**How do I install it on Linux?**

Each [release](https://github.com/obs-rust/obs-studio/releases) has an
AppImage, which runs on any distribution (`chmod +x` it and run it), and a
.deb for Ubuntu 26.04 (`sudo apt install ./OBS-Studio-Rust-*.deb`).
The AppImage is tested on AlmaLinux 8 (the glibc 2.28 floor), Ubuntu 22.04,
Ubuntu 24.04, Arch Linux and NixOS. Each release also has a Flatpak
bundle: `flatpak install --user ./OBS-Studio-Rust-*-linux-x86_64.flatpak`,
then `flatpak run io.github.obs_rust.OBSStudioRust`. It gets the Freedesktop
runtime from Flathub and doesn't update itself yet; install the next
release's bundle the same way. It isn't on Flathub: `com.obsproject.Studio`
there is upstream OBS Studio, so report its bugs there. Ours installs as
`io.github.obs_rust.OBSStudioRust`, beside upstream's rather than over it.
It keeps its settings in its own folder, so to bring over your scenes from
upstream's Flatpak, copy `~/.var/app/com.obsproject.Studio/config/obs-studio`
to `~/.var/app/io.github.obs_rust.OBSStudioRust/config/obs-studio`. Its
plug-ins go in `~/.var/app/io.github.obs_rust.OBSStudioRust/data/obs-studio/plugins/`.

**How do I run the AppImage on NixOS?**

The simplest way is `appimage-run` (`programs.appimage.enable = true;`, then
`appimage-run ./OBS-Studio-Rust-*.AppImage`). It gives the AppImage the
paths a regular distribution has, and OBS renders on your GPU and finds NVENC.

Running the AppImage directly works through nix-ld. Your configuration needs
`hardware.graphics.enable = true;`, `programs.nix-ld.enable = true;` and
Wayland among nix-ld's libraries:

```nix
programs.nix-ld.libraries = with pkgs; [ wayland ];
```

Without Wayland there, the AppImage stops at start and says so. You don't
need to add `/run/opengl-driver/lib` or the keyboard layouts: the AppImage
finds the driver libraries (for NVENC) and `/etc/X11/xkb` itself.

**How do I install plug-ins for the AppImage?**

The AppImage loads plug-ins from the same per-user folders as OBS Studio on
Linux, so a plug-in you already installed for your own account is picked up
as-is. To add one, unpack its Linux build into either of these folders:

- `~/.local/share/obs-studio/plugins/<plugin>/`, with the module at
  `<plugin>/<plugin>.so` and its files under `<plugin>/data/` (the current
  layout).
- `~/.config/obs-studio/plugins/<plugin>/`, with the module at
  `<plugin>/bin/64bit/<plugin>.so` and its files under `<plugin>/data/` (the
  older layout that many plug-in downloads still use).

Then restart OBS Rust. **Tools → Plugin Manager** lists what loaded.

Plug-ins installed system-wide by a distribution package (under
`/usr/lib/obs-plugins` or `/usr/lib/<arch>/obs-plugins`) are **not** loaded.
They were built against that distribution's OBS and libraries, not the ones
inside the AppImage. Use the plug-in's own Linux download instead.

A plug-in with its own Qt windows must be built for a Qt 6 release no newer
than the one in the AppImage. If a plug-in sits in one of those folders but
doesn't load, run the AppImage from a terminal and the log will say why.
Then file an issue as above.

Want to see the original README from OBS Project? [Click here](README.upstream.md).

# 🏆 OBS Project User Abuse Hall of Fame

Bugs they confirmed, then closed. Features you relied on, then removed.
Twelve upstream threads where users showed up in numbers and were turned
away anyway. Six of them were closed as stale on a single afternoon.

| | Issue | 💬 | Fate |
|:-:|---|--:|---|
| 🥇 | [Preview Stutter that appears in Streams/Recordings](https://github.com/obsproject/obs-studio/issues/4191) | 184 | locked (off-topic) |
| 🥈 | [OBS Virtual Camera may affect built-in webcams](https://github.com/obsproject/obs-studio/issues/3555) | 87 | closed not planned, locked |
| 🥉 | [Removal of --disable-shutdown-check breaks automated background workflows](https://github.com/obsproject/obs-studio/issues/12650) | 64 | closed not planned, locked |
| 4 | [Dock Sizes are Reset the next time you open OBS.](https://github.com/obsproject/obs-studio/issues/2949) | 34 | closed not planned |
| 5 | [Pause and Resume Recording Doubles or Triples CPU usage](https://github.com/obsproject/obs-studio/issues/3148) | 34 | closed not planned |

**In their own words:**

> **“No amount of "I don't like your answer" will change it.”**
> — [#9285](https://github.com/obsproject/obs-studio/issues/9285#issuecomment-1677822114), to a user who said a downgrade fixed their dropped frames

> **“However, if you refuse to follow the process, there is nothing we can do, and this report is not actionable.”**
> — [#13685](https://github.com/obsproject/obs-studio/issues/13685#issuecomment-5052743359), in the same comment that admitted 32.2 had raised the driver minimum

> **“[…] that decision has already been made.”**
> — [#12650](https://github.com/obsproject/obs-studio/issues/12650#issuecomment-3368592277), on removing a flag users' setups depended on

**[See all 12, every quote, and why each one made the cut →](docs/hall-of-fame.md)**

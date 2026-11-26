# Flatpak OBS compiler cache decision

Use option 3. Stage the pinned static musl zccache archive in the OBS module
and pass its sandbox path through the existing CMake `OBS_ZCCACHE_LAUNCHER`
override for C and C++. This is the only module being migrated. A build-only
SDK or manifest module (option 1) would add deployment and cleanup work for a
tool that no other module uses. Exposing a runner binary (option 2) would
require adding runner-specific permissions to the builder's separate
sandboxes. The release archives are outside the app prefix and the
`Zccache Stats` step rejects staged tools or state under the installed
`files/` tree.

The manifest pins both [zccache 1.15.22 release archives](https://github.com/forge-ci/zccache/releases/tag/1.15.22).
Both downloaded tarballs were checked with `sha256sum`. The manifest
regression test asserts the exact digest and architecture of each archive.

| Architecture | SHA-256 |
| --- | --- |
| x86_64 | `11bf3c33a55c9032f6e153faa21c06d35f137e9ccbe13a91c3a7daa24f56f65d` |
| aarch64 | `e4cd3f7123e890f04ef157be375c6c0a90203a675e373cc68a87e14b03f671a4` |

The build runs in the Freedesktop 26.08 SDK. Flatpak Builder binds the
module source at `/run/build/obs`, which makes the launcher visible as
`/run/build/obs/.flatpak-zccache/zccache`. We keep the
[pinned action's `cache: true`](https://github.com/flatpak/flatpak-github-actions/blob/79327416609af08178ad73b352877e51450790b3/flatpak-builder/index.js#L247-L249)
to keep SDK ccache available to dependency modules. zccache uses the builder's
[writable `/run/ccache` bind mount](https://github.com/flatpak/flatpak-builder/blob/1.4.8/src/builder-module.c#L1266-L1270).
The builder also creates the mount with ccache disabled. OBS selects
`/usr/bin/cc` and `/usr/bin/c++` directly.

The OBS Ninja build uses `/run/ccache/obs-zccache/state` for content-addressed
data and the socket at `v1.15.22/daemon-v1.15.22.sock` beneath that root.
The host sees the state at `.flatpak-builder/ccache/obs-zccache/state`.
An isolated builder state therefore also isolates OBS's cache. When the
build environment lacks the writable bind mount, the helper uses a
module-local state directory outside `/app`.

Flatpak Builder launches configure, Ninja and install in separate
sandboxes. The manifest stages a Ninja entry point that delegates to a
CMake script in the OBS module. The script opens one fresh zccache session for the parallel Ninja compile
invocation, ends the session, and stops the daemon in that same sandbox.
This follows the session finalization and cache flush in
[`build-aux/appimage/build.py`](../build-aux/appimage/build.py) and the
cache lifecycle in [`rust/tools/linux-validate/validate.sh`](../rust/tools/linux-validate/validate.sh).
Only successful compilation and shutdown publish the session report.
The opt-in diagnostics upload a bounded, sanitized journal summary.

Production builds keep Actions cache restore and save disabled. The
explicit compiler transfer probe archives durable state only after the
session report exists. It retains source, SDK commit, compiler digest,
architecture and manifest identity checks and adds the release digest.
The source-fetch and pruned-builder probes still isolate dependency and
compiler state. The warm and cross-runner probes require at least 100
hits and at least 80% hits among cacheable compilations.

The hosted `flatpak-build` job in `.github/workflows/build-project.yaml`
must exercise the SDK's Ninja entry point, launcher and daemon shutdown.
Its `Zccache Stats` line reports warm hits, cold misses and uncacheable
compiles for that session. Zero hits emits a warning on a cold build;
missing or empty session evidence fails. When reuse is expected the step
runs with `--expect-reuse` and zero hits fails: the compiler-transfer
consumer always passes it, and the main build passes it only for the
`flatpak-expect-compiler-reuse` dispatch input (#5788). Run IDs cited as
evidence must appear in [the evidence ledger](evidence/zccache-flatpak.json),
which `ci/test_zccache.py` checks. The opt-in same-runner and cross-runner probes
qualify warm hits, and the saved-profile runtime job qualifies the
installed export. Run `ci/flatpak_pch_probe.py --zccache /path/to/zccache
--output /path/to/evidence` in the matching SDK to repeat the same-size,
same-timestamp freshness trials with the new launcher. Historical
[ccache results](ci-speed-flatpak-pch-1760.md) describe the previous lane.
The current workflow and Rust toolchain module target x86_64. The
aarch64 launcher pin is ready for SDK qualification on that architecture.

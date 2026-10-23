# Cargo build handoff (#1332)

## Current state

The completed milestones are merged into master:

- #1805: shared typed native test graph.
- #1844: C++ standards and six unchanged frontend utility tests.
- #1853: canonical Qt generation, runtime environment handling, and the
  unchanged remux queue test through both build consumers.
- #1856: bundled native builds follow effective Cargo optimization and debug
  information settings, with explicit native assertion overrides. Native
  macOS Swift assertion behavior is covered too.

PR and merge-result CI passed for #1856. This handoff adds documentation only;
it contains no new SDK resolver, implementation, or test results. CMake remains
authoritative and #1332 remains open.

## Next unit: explicit Qt SDK interfaces

The shared graph can generate Qt code, but SDK discovery still depends on
pkg-config. `rust/obs-build/src/qt.rs` chooses tools from explicit
`OBS_QT_MOC`, `OBS_QT_UIC`, and `OBS_QT_RCC` overrides or Qt6Core's pkg-config
`libexecdir`. `libobs::add_package` resolves generic package interfaces.
This does not establish native Windows import libraries or macOS framework
interfaces for the complete Cargo application.

Suggested next steps, requiring implementation and validation:

1. Define explicit target SDK and host tool inputs. Cross builds must not
   silently select host libraries through ambient pkg-config.
2. Resolve canonical Qt Core, Gui, and Widgets interfaces, including transitive
   dependencies, compile options, link artifacts, and runtime paths. Keep OS
   details behind `src/platform/`.
3. Validate SDK version, architecture, configuration, library naming and
   layout; fail clearly on missing or incompatible inputs. Determine debug
   library/CRT and framework behavior from real SDKs rather than guessed paths.
4. Bind moc/uic/rcc to the chosen host SDK without process-wide environment
   mutation. Reuse the graph's runtime-path interfaces for DLLs, frameworks,
   and plugins as appropriate.
5. Start with RED tests in the existing application test binary, then prove
   generation and the unchanged remux queue test against real SDKs. Cover
   native Windows/macOS using existing CI lanes; do not add workflow files.

Useful entry points: `rust/obs-build/src/validation/graph.rs` (`Dependency`,
`Test::resolve_runtime`), `validation.rs` (`build_qt_tests`), platform libobs
modules, and `ci/linux_qt_frontend.sh` (explicit prebuilt Qt prefix).

Qt's [qmake query properties](https://doc.qt.io/qt-6/qmake-environment-reference.html),
[host-tool SDK setting](https://doc.qt.io/qt-6/cmake-variable-qt-host-path.html),
and [qt.conf path rules](https://doc.qt.io/qt-6/qt-conf.html) are research leads,
not evidence that a resolver has been implemented or tested.

## Remaining parent acceptance

Qt SDK resolution alone will not finish #1332. Remaining work includes the
complete native application graph on supported platforms, frontend/plugins and
generated assets, runtime layout and packaging, native compiler/cache and
unity/PCH measurements, application and protocol compatibility, and packaged
artifact tests using representative saved capture profiles. Retire CMake's
authority only after the parent issue's complete acceptance criteria pass.

## Working constraints

Work serially in the existing checkout; do not create Git worktrees. Preserve
the unrelated local `plugins/obs-browser` submodule mismatch. Use `soldr cargo`
and `uv run`/`uvx`, preserve unchanged C tests and public headers, and keep the
eight existing test categories. No admin merges or new workflow files.
Do not modify sources during live bind-mounted Docker validation.

The outgoing agent is stopping at this handoff; the next agent should claim
the linked task before implementing it. Nothing in this draft should be read
as a claim that the full Cargo build migration is complete.

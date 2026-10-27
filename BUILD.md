# Building C, C++ and Objective-C: unity groups and precompiled headers

This is the build policy for every first-party C, C++ and Objective-C
source in this repository (`libobs/`, `libobs-*/`, `frontend/`, `plugins/`,
`shared/`). It exists so the native build is as fast as we can make it while
staying correct on every platform and every CMake generator. Rust has its
own rules in [docs/rust-port/testing-policy.md](docs/rust-port/testing-policy.md).

Out of scope: third-party code (`deps/`), the git submodules
(`plugins/obs-browser`, `plugins/obs-websocket`), the unchanged C tests
(`test/`) and the C oracle (`rust/obs-c-oracle/`). They build the way their
upstream ships them.

## Why

Compiling one translation unit per source re-parses the same headers
hundreds of times. Measured on a fresh Linux tree (`-j16`, no compiler
cache):

| Build | Wall | CPU |
|---|---|---|
| One TU per source | 315 s | 2416 s |
| Precompiled headers | 322 s | 1798 s |
| Precompiled headers + unity | 198 s | 1053 s |

Those are broader native-build measurements, not isolated libobs timings.
A separate libobs-only experiment (GCC 15.2, RelWithDebInfo, `-j8`, three
trials per condition) measured uncached medians of 9.75 → 9.41 s wall and
63.48 → 46.62 s CPU; the wall-time ranges overlap. Warm ccache rebuilds
measured 1.54 → 1.44 s, with all cacheable compilations hitting. See the
[scoped results, raw evidence and reproduction commands](docs/ci-cache-audit-2026-10-17.md#isolated-c-libobs-build)
before applying either result to CI runner or full-app timing claims.

CMake's own `UNITY_BUILD` got us those numbers but batches sources by count
in list order. Each OS has a different source list, so each OS gets different
batches, and a name clash between two files appears only on the platform
where they happen to share a batch: Linux built clean while Windows failed
in `libobs-d3d11`, `nv-filters`, `text-freetype2` and `win-dshow` (#1128,
#1139). A precompiled header force-included in front of files that also
include their own headers hides what each file really depends on: a header
that only compiled thanks to the PCH broke on macOS alone, and a lean
`<windows.h>` from the PCH broke files that needed the full one on Windows
arm64. Per-file skip lists in CMake piled up to work around both.

The fix is to make the grouping and the headers part of the source tree
instead of build-system heuristics: the same files share a translation unit,
behind the same single header, on every compiler, OS and generator.

## The rules

### 1. File names say the role

| Role | C | C++ | Objective-C | Objective-C++ |
|---|---|---|---|---|
| Implementation (the code) | `foo.c.h` | `foo.cpp.hpp` | `foo.m.h` | `foo.mm.hpp` |
| Group (what the build compiles) | `name+.h.c` | `name+.hpp.cpp` | `name+.h.m` | `name+.hpp.mm` |
| Precompiled header | `family.pch.h` | `family.pch.hpp` | `family.pch.h` | `family.pch.hpp` |
| Header (declarations) | `foo.h` | `foo.hpp` | `foo.h` | `foo.h` |

An implementation file is ordinary source; the build reaches it only through
a group. C-family files end in `.h` and C++-family files in `.hpp` because
tools pick the language from the last extension: clang-format formats a
`.hpp` as C++, where names C code uses (`private`, `new`) are keywords. Group files and precompiled headers live in a `build/` directory
next to the component's `CMakeLists.txt` (`libobs/build/`). A group contains
its precompiled header, then its implementation files in alphabetical order:

```c
/* libobs/build/libobs.util+.h.c */
/* Unity group (BUILD.md): util/ and callback/. */

#include "libobs/build/libobs.pch.h"

#include "libobs/callback/calldata.c.h"
#include "libobs/callback/decl.c.h"
#include "libobs/util/cf-lexer.c.h"
```

Every quoted include of a repo file in a group, a precompiled header, an
implementation file or a forwarding file is a repo-root path
(`plugins/obs-ffmpeg/obs-ffmpeg-formats.h`), never one relative to the file
(`../obs-ffmpeg-formats.h`): a line reads the same wherever it is, and moves
with no rewriting. `cmake/common/groups.cmake` makes the repo root an include
directory of every target. Headers found through a target's own include
directories (`<obs-module.h>`, `"util/base.h"`) keep their usual form.

### 2. One header per group, and it is the only include

Every implementation file includes exactly one header: its group's
precompiled header, as its first line.

```c
/* libobs/util/cf-lexer.c.h */
#include "libobs/build/libobs.pch.h"

static inline void cf_convert_from_escape_literal(...)
```

Everything a file needs (system headers, the component's own headers, the
feature macros that must precede them such as `_FILE_OFFSET_BITS` or
`WIN32_LEAN_AND_MEAN`, and the platform conditionals around them) lives in
that header, in the order the files need it. A file's own macros (`do_log`, a
`FALLBACK_FRAMERATE`) stay in the file, right after its include: only the
macros that set up headers move. A header that defines macros of its own
(obs-outputs' `rtmp-stream.h` defines `do_log`) puts its includers in a
family of their own, and a header that defines data (`mp4-mux-internal.h`)
stays out of headers the C tests compile more than once. So the precompiled header is
exactly what its files depend on, every member of a group sees the same
thing whether it is compiled in the group or alone, and nothing is
force-included in front of anything.

Groups share a header when their files need the same things (all of
libobs's core groups use `libobs.pch.h`), which is what makes precompiling it
pay. A group gets a header of its own where its dependencies differ:

- third-party headers some files must not pull in (FFmpeg for
  `libobs-media.pch.h`; the Rust-ported files, which the C oracle and the C
  tests compile on their own, have `libobs-ported-*.pch.h` without them),
- a platform (`libobs-windows.pch.h`, `libobs-nix.pch.h`), named so
  `ci/platform_ifdef_ratchet.py` sees its host,
- an optional feature (`libobs-pulse.pch.h`, `libobs-wayland.pch.h`),
- a feature macro that must come first (`libobs-platform.pch.h` for
  `_FILE_OFFSET_BITS`, `libobs-ported-posix-threading.pch.h` for
  `_GNU_SOURCE`),
- a different `<windows.h>` (lean or full).

One language per group: C, C++, Objective-C and Objective-C++ files never
share one. Platform-only groups are listed in the component's
`cmake/os-<os>.cmake` like any platform source, with no `#ifdef` choosing
between groups.

Two exceptions, both after the code starts:

- **No-op host capabilities** (`libobs/obs-host/noop/<capability>.hpp`,
  #1051) are headers, not group members: each host file
  (`obs-nix.c.h`, `obs-windows.c.h`, `obs-cocoa.m.h`) includes the
  ones for the capabilities it lacks, at its end.
- **X-macro tables** (a header without `#pragma once` or an include guard,
  such as `obs-hotkeys.h`) included as data.

A C test that `#include`s a source file by its old name (`#include "media.c"`)
keeps working through a one-line forwarding file left at that name
(`media.c`: `#include "shared/media-playback/media-playback/media.c.h"`). The build never compiles it, and the
unchanged C test needs no edit.

### 3. Unity and precompiled headers are always on

Converted components build one way: each group is one translation unit, and
the header most of a target's groups share is precompiled (per language; on
Xcode, which allows one prefix header per target, for the language with the
most groups). A group of another family includes its header textually.
`ENABLE_UNITY_BUILD` and `ENABLE_PCH` only steer components not yet
converted.

A target that compiles one implementation file directly (a C test pinning a
single libobs file) compiles a generated one-line wrapper with a real
extension that includes it (generators, Xcode among them, take a source's
type from its extension), with the owning target's include directories,
since its header comes from there.

### 4. Every implementation file is in exactly one group

A file in no group silently drops out of the build; a file in two defines
its symbols twice. It may be in several groups only when each is for a
different host. `ci/test_build_groups.py` (part of `./lint`) checks this,
the shape of every group, and rule 2. New code joins a group in the same
commit that adds the file.

### 5. Names that are file-local must be unique within a group

Inside a group, every implementation file shares one translation unit, so:

- `static` functions and variables, file-scope `enum` enumerators and
  anonymous-namespace names get a prefix from their file
  (`od_` for `obs-output-delay`, `compressor_` for the compressor filter).
  An anonymous namespace does not help: the files share it.
- A file-scope `#define` is `#undef`ed at the end of the file
  (`ci/test_build_groups.py` checks it; a macro a header provides may be
  restored there instead). A macro that
  shadows a system name (`near`, `far`) is saved and restored with
  `push_macro`/`pop_macro`.
- Default arguments are declared once, in the header.
- Headers have `#pragma once` and are never duplicated under two paths (a
  copy forwards to the original).
- No `using namespace std;` in a group whose header brings in global names
  `std` also declares (`byte` from the Windows SDK): qualify instead.

When a clash is cheaper to isolate than to rename (two backends that
deliberately define the same functions, a vendored fork), the file gets its
own group. That is the only exemption, and it lives in the source tree, not
in CMake.

### 6. Renames are mechanical and in place

Turning `foo.c` into `foo.c.h` keeps the directory and the stem, so
`git log --follow`, `git blame` and merges from `upstream/master` follow
the file. Each component converts in its own PR with this order of commits:

1. Only `git mv` of the implementation files (no content change), so rename
   detection is exact.
2. The groups, their precompiled headers (each file's includes moved into
   its group's header), the CMake change, and path updates in `ci/`, the
   platform-ifdef baseline, the C tests' CMake and `rust/` (oracle includes).
3. Any clash fixes.

When an upstream merge adds a new `foo.c` to a converted component, rename
it, move its includes into its group's header and add it to a group in the
merge PR.

A newly ported libobs unity group can register itself in
`libobs/build/ported-source-sets/<unit>.json` with the same
`LIBOBS_*_PORTED_SOURCES` keys as `libobs/build/source-sets.json`. The CMake,
Cargo-native and Tier 2 inventory readers discover these fragments, so two
ports can add their own files without editing the same ported-source list.
Keep each member in exactly one list or fragment.

## Verifying a change

- Linux: `./unittest` and the local gate as usual; `./lint` runs
  `ci/test_build_groups.py`.
- Windows and macOS from Linux: `cmake/cross` (soldr's clang with the
  Windows SDK and the Apple SDK) builds libobs and the platform plugins
  through the same groups. Extend it when a component converts.
- A change to `cmake/common/`, `cmake/pch/` or a precompiled header itself
  must pass a full Windows and macOS build
  (`gh workflow run ci.yml --ref <branch>`, which runs every build) before
  it merges. PR CI never builds the OBS apps: `cmake/common/` starts only
  the macOS and Windows Rust tests and Tier 2, `cmake/pch/` starts nothing
  beyond the Fast Gate, and a `libobs/build/` file starts Linux Tier 2. A
  port that only adds its source's `#include` to a `ported-*` group does
  not need the manual run.

## Transition

Components not yet converted still build with CMake's `UNITY_BUILD`
batching, the shared headers in `cmake/pch/` and the skip lists in
`cmake/common/unity.cmake` and `cmake/common/pch.cmake`. Those lists only
shrink: a new entry means the file's component should be converted instead.
When the last component converts, the batching, `cmake/pch/` and the skip
lists are deleted. Order: `libobs/` (done), then `libobs-*/`, `shared/`,
`plugins/`, `frontend/`.

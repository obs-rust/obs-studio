# Public-header platform conditional audit (#878)

> The `docs/ci-evidence/` snapshots linked below are retired from `master`
> (#2686); each link resolves to the commit that last held the file
> (`c2af3f9c3`), and the commands quoted below name those paths as they were
> then.

Audited base: `c9b06b371e45833fb55b686b78904ae246311be9`. All 33 issue-listed platform conditionals remain in
14 headers. This change records their interface reasons and existing ratchet
protection; it changes no header, declaration, layout, inline implementation,
export or build option. The [machine-readable inventory](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-21-public-header-conditionals.json)
records exact Git blobs, expressions and current line numbers.

## Protection and scope

- Seven headers already have reasoned entries in
  `ci/platform-ifdef-exceptions.toml`. Those entries are unchanged. The exception
  list only shrinks; this audit adds no exemption.
- The remaining seven have per-file limits in `ci/platform-ifdef-baseline.json`.
  Keep these stricter bounds: device-exports.h (2), audio-math.h (2), base.h (1),
  curl-helper.h (1), dstr.h (1), pipe.h (1), profiler.h (1). Their reasons are
  recorded below and alongside the exception configuration.
- `graphics/device-exports.h` and `util/curl/curl-helper.h` are **not** in
  `libobs/CMakeLists.txt`'s common `public_headers` list. The issue includes them
  as header interfaces; this audit does not reclassify them as installed public
  API or grant them a broad public-header exemption.
- The 33 count is the issue's host-conditional inventory, using the ratchet's
  macro families. The ratchet also counts `<objc/objc.h>` in graphics.h, giving
  **34 findings**. Compiler-only branches such as nested `__MINGW32__` printf
  archetype selection are retained too, but are outside that host count.
- Rust replacements do not remove the C header contract: external C/C++ callers
  still compile these declarations and inline helpers. No header is rewritten
  to accommodate a Rust implementation.

## Inventory

Line numbers refer to the audited base and are observations, not permanent IDs.
Expressions are recorded verbatim; the JSON retains unescaped text.

| Header and line | Conditional | Interface reason retained |
| --- | --- | --- |
| `libobs/graphics/device-exports.h:137` | `#ifdef __APPLE__` | Apple IOSurface/shared-texture device exports |
| `libobs/graphics/device-exports.h:142` | `#if defined(__linux__) &#124;&#124; defined(__FreeBSD__) &#124;&#124; defined(__DragonFly__)` | Linux/FreeBSD/DragonFly dma-buf device exports |
| `libobs/graphics/graphics.h:22` | `#ifdef __APPLE__` | Apple Objective-C runtime types |
| `libobs/graphics/graphics.h:481` | `#if defined(_WIN32)` | Windows gs_window hwnd member |
| `libobs/graphics/graphics.h:483` | `#elif defined(__APPLE__)` | Apple gs_window NSView id member |
| `libobs/graphics/graphics.h:485` | `#elif defined(__linux__) &#124;&#124; defined(__FreeBSD__)` | Linux/FreeBSD gs_window X11 id/display members |
| `libobs/graphics/graphics.h:819` | `#ifdef __APPLE__` | Apple IOSurface/shared-texture API |
| `libobs/graphics/graphics.h:828` | `#elif defined(_WIN32)` | Windows GDI/shared-texture API |
| `libobs/graphics/graphics.h:890` | `#elif defined(__linux__) &#124;&#124; defined(__FreeBSD__) &#124;&#124; defined(__DragonFly__)` | Linux/FreeBSD/DragonFly dma-buf API |
| `libobs/media-io/audio-math.h:23` | `#ifdef _MSC_VER` | MSVC float declarations and warning push for inline audio math |
| `libobs/media-io/audio-math.h:41` | `#ifdef _MSC_VER` | MSVC warning pop paired with the push |
| `libobs/obs.h:431` | `#ifdef _WIN32` | Windows-only obs_init_win32_crash_handler export |
| `libobs/util/base.h:79` | `#if !defined(_MSC_VER) && !defined(SWIG)` | Format attributes on public variadic logging functions; disabled for MSVC/SWIG |
| `libobs/util/c99defs.h:26` | `#ifdef _MSC_VER` | Compiler-specific deprecation, noreturn and force-inline declarations |
| `libobs/util/c99defs.h:48` | `#if defined(_MSC_VER) &#124;&#124; defined(__MINGW32__)` | Windows dllexport versus ELF/Mach-O default visibility |
| `libobs/util/c99defs.h:54` | `#ifdef _MSC_VER` | Compiler-specific diagnostic pragma macros |
| `libobs/util/curl/curl-helper.h:21` | `#if defined(_WIN32) && LIBCURL_VERSION_NUM >= 0x072c00` | Windows Schannel revocation-option macro for supported libcurl versions |
| `libobs/util/dstr.h:42` | `#ifndef _MSC_VER` | Format attributes on public string formatting declarations |
| `libobs/util/pipe.h:41` | `#ifndef _MSC_VER` | Format attribute on process_args_add_argf |
| `libobs/util/platform.h:37` | `#ifdef _WIN32` | Windows exported os_stat function versus stat macro elsewhere |
| `libobs/util/platform.h:203` | `#ifdef __APPLE__` | Apple ARCH_BITS selection |
| `libobs/util/platform.h:206` | `# ifdef _WIN32` | Windows ARCH_BITS selection inside non-Apple branch |
| `libobs/util/platform.h:207` | `# ifdef _WIN64` | Windows 64-bit versus 32-bit ARCH_BITS selection |
| `libobs/util/profiler.h:43` | `#ifndef _MSC_VER` | Format attributes on public profiler declarations |
| `libobs/util/sse-intrin.h:22` | `#if (defined(_MSC_VER) &#124;&#124; defined(__MINGW32__)) && ((defined(_M_X64) && !defined(_M_ARM64EC)) &#124;&#124; defined(_M_IX86))` | Native x86 MSVC/MinGW intrinsics versus SIMDe, excluding ARM64EC |
| `libobs/util/sse-intrin.h:29` | `#if defined(_MSC_VER) && defined(__cplusplus)` | MSVC C++ cmath dependency for SIMDe |
| `libobs/util/sse-intrin.h:33` | `#if defined(__APPLE__)` | Apple simd/base.h dependency |
| `libobs/util/threading.h:29` | `#ifndef _MSC_VER` | Non-MSVC errno dependency |
| `libobs/util/threading.h:41` | `#ifdef _MSC_VER` | MSVC intrinsics versus POSIX threading inline implementation |
| `libobs/util/threading.h:98` | `#ifdef _MSC_VER` | MSVC __declspec(thread) versus __thread storage declaration |
| `libobs/util/util_uint64.h:19` | `#if defined(_MSC_VER) && defined(_M_X64)` | MSVC x64 intrinsic declarations |
| `libobs/util/util_uint64.h:25` | `#if defined(_MSC_VER) && defined(_M_X64) && (_MSC_VER >= 1920) && !defined(__clang__)` | MSVC >=1920 native x64 _umul128/_udiv128 implementation excluding clang-cl |
| `libobs/util/util_uint64.h:30` | `#elif defined(_MSC_VER) && defined(_M_X64) && defined(__clang__)` | clang-cl x64 unsigned __int128 fallback |

## ABI preservation evidence

The audit has no compiled-code change. For this PR, exact Git identity of the
**entire libobs tree** against the audited base proves that its public headers,
implementation, exports and libobs build configuration are unchanged. This is
stronger than checking a subset of symbols or layouts for this documentation
change; it is not a claim that all possible C/Rust ABI parity is independently
revalidated here.

```sh
git diff --exit-code BASE HEAD -- libobs
uv run ci/platform_ifdef_ratchet.py
uv run python -m unittest ci.test_platform_ifdef_ratchet
```

Replace `BASE` with the audited commit above. Compare each recorded header's
blob with `git rev-parse HEAD:path` and the recorded `libobs_tree` with
`git rev-parse HEAD:libobs`. No public-header ABI checker is invented or skipped:
byte-for-byte identity is the ABI-preservation check for this no-code-change
PR. Native build and Rust port tiers are not applicable to documentation and
TOML comments.

## Outcome

All 33 issue-listed conditionals are recorded with their reasons and existing
per-file protection. No condition is moved, no broad exception is added, and no
baseline is raised. These branches remain under the documented public/header
interface policy; the wider #847 facade cleanup continues independently.

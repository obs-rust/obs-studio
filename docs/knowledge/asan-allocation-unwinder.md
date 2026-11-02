---
type: concept
title: Which asan tests keep the slow allocation unwinder
description: "Distro FFmpeg has no frame pointers, so the fast unwinder stops at av_malloc; only tests that link FFmpeg keep the slow one, and a test the scan does not find stays slow."
sources: ["#2035", "#3249", "#3365", "#3456"]
status: active
---

# Which asan tests keep the slow allocation unwinder

## The trap

`fast_unwind_on_malloc=0` makes ASan unwind every allocation the slow way, which is
most of the time that allocation-heavy tests spend. `=1` is much faster, but it
follows frame pointers, and FFmpeg on ubuntu-26.04 has none. `objdump` of
libavutil and libavformat confirms this. So a leak allocated inside FFmpeg shows a
stack that stops at `av_malloc`, and triage cannot name an OBS location.

## How it showed up

- #2035: stacks cut at `av_malloc` led to the global `fast_unwind_on_malloc=0`.
- #3249: with `=1` for the whole pass, the unit tests took 302 s against 366 s, and
  `test_context_names` went from 29 s to 2 s. The stacks of `test_media_alpha_network`
  (#3365) went from 10 to 13 frames down to 2.
- #3456 then split it per test: `UnwindPolicy` in `rust/tools/sanitize/sanitize.py`.

## The rule

A test gets the fast unwinder only in the asan pass, and only when all of this holds:

- The scan of the test's build definition found it.
- It does not link `FFmpeg::*` or `OBS::media-playback` (`FFMPEG_LIBRARY`).
- It is not in `SLOW_UNWIND_TESTS`. `test_core_module_load` is, because it failed
  24 of 30 runs under load with the fast unwinder (`XOpenDisplay` during hotkey
  initialization, #3249). The cause is not known.

A test the scan does not find keeps the slow unwinder. The app launch always uses
the slow one.

## What to do instead

If a new leak's stack stops at `av_malloc` or looks cut off, check which unwinder
the test used before you look for a bug in the report. Do not add a frame-pointer
assumption about a system library without `objdump`: that assumption was wrong once
on #3249.

## Checks

`test_sanitize_harness.py` covers which tests keep the slow unwinder.

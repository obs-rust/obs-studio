---
type: concept
title: Free what OBS's destroy path frees
description: A test that skips OBS's destroy call leaks what that call frees, and the asan job fails on that since #3290.
sources: ["#3400", "#3401", "#3290"]
status: active
---

# Free what OBS's destroy path frees

## The trap

A unit test drives one internal function on a struct it built itself (on the
stack, or with only part of the create call). OBS never leaks there, because
its destroy or close path frees what the function allocated. The test does not
go through that path, so it leaks. Tier 2 does not check leaks, so it passes
there; LeakSanitizer and valgrind report it.

## How it showed up

- #3400: `test_ffmpeg_mux_write_packet` leaked the 64 KiB `stream->packet_buf`
  that `write_packet()` grows with `da_*`. OBS frees it in
  `ffmpeg_mux_destroy()`; the test's stream never went there. Fix: `da_free()`
  the buffer at the end of the test.
- #3401: `test_rtmp_send_batch` leaked the per-channel packets
  `RTMP_SendPacket()` keeps. OBS calls `RTMP_Close()` (which frees
  `m_vecChannelsOut`) on every disconnect; the test only called `RTMP_Free()`.
  Fix: `RTMP_Close()` before `RTMP_Free()`.

## What to do instead

Before writing such a test, read the destroy/close path of the object you
touch and free exactly what it frees, the way it frees it (call the same close
function where you can, rather than freeing fields by hand). Do not free more:
that hides a real leak in OBS. Never change an assertion to fix a leak.

## Checks

- The `asan` job in `sanitizers.yaml` runs with `--fail-on-findings` since
  #3290, so a LeakSanitizer report from a test fails it.
- A leak you cannot fix in the test belongs in the known list only as a
  tracked `bug` (see [sanitizer-known-list.md](sanitizer-known-list.md)).

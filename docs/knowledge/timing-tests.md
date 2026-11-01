---
type: concept
title: Timing tests wait for the result
description: Wait for the frame or packet you check, not a frame count or a fixed sleep; when a test needs the `timing` resource lock.
sources: ["#3389", "#3390", "#3421", "#3290"]
status: active
---

# Timing tests wait for the result

## The trap

A test that renders a scene or plays media checks the output after "enough"
frames or after a fixed time. On an idle machine that is enough; in the asan
pool (every core busy, ASan slowing each process) it is not, and the test reads
output from before the thing it checks happened. `testing-policy.md` says "No
sleeps: synchronize on events"; a frame count is a sleep measured in frames.

## How it showed up

- #3389: `test_darken_transparent`'s `run_scene` was "ready" after 10 output
  frames, not after a frame that showed the scene. Under load the first pass
  measured luma 0 (no frame drawn yet, not even video black 16).
- #3390: `test_deinterlace_mode_load` pushed at most 90 frames (3 s) for Yadif
  2x to show the source; under ASan it needed close to all of them.
- #3421: `test_media_cover_art_monitor` plays real media in real time; in the
  pool the first play got 0 packets inside its window.
- #3290: `test_slideshow_mem_cap` waited out a 5 s timeout on each of 12
  presses that never landed (100 s, now 42 s once each press waits for its picture).

## What to do instead

1. Wait on the condition the assertion checks: poll output frames until one
   shows the scene (for example, a pixel that is not black), or until the
   packet you count has arrived, with a generous upper bound that only turns a
   hang into a failure. Report how many frames or how long it took on failure.
2. Never make the bound the thing that passes the test.
3. If the test inherently depends on wall-clock (real-time playback, an audio
   tone, a dock layout settling), add it to the `timing` resource lock:
   `set_tests_properties(<test> PROPERTIES RESOURCE_LOCK timing)` in its
   `test/cmocka/tests.d/<test>.cmake`, or `resource_locks: ["timing"]` in
   `test/cmocka/build/test-graph.json`. The sanitizer pool then runs it alone
   after the others; in Tier 2 ctest only keeps such tests from overlapping.
   Add the name to `TIMING_TESTS_D`/`TIMING_TESTS_GRAPH` in
   `ci/test_sanitize_harness.py`.

The lock is a mitigation, not a fix: a test that waits for its result
does not need it.

## Checks

- `ci/test_sanitize_harness.py` `TimingLockTest`: the listed timing tests hold the lock.
- The asan job (`sanitizers.yaml`) fails on a unit-test failure since #3290.

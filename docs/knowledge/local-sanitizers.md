---
type: concept
title: Running sanitizers locally
description: Asan or valgrind on a few tests through the harness on a local machine, and what valgrind costs per test.
sources: ["#3290", "#3238", "#962"]
status: active
stale_after: 2027-05-01
---

# Running sanitizers locally

## The harness

`rust/tools/sanitize/sanitize.py` is what CI runs (`sanitizers.yaml`):

```sh
rust/tools/sanitize/install_ubuntu_deps.py      # Ubuntu, once
uv run --no-project rust/tools/sanitize/sanitize.py asan      # or tsan / valgrind
uv run --no-project rust/tools/sanitize/triage.py rust/tools/sanitize/out/asan
```

It configures and builds OBS for the tool in `$SAN_OUT/build-<tool>` (default
`rust/tools/sanitize/out/`), runs every registered cmocka test under it, then
launches the app headless. Results go to `$SAN_OUT/<tool>`. `SANITIZE_JOBS`
sets how many tests run at once (`1` runs serially on one shared display);
`SKIP_TESTS=1` only relaunches the app.

## A few tests

The harness has no test filter: one full run builds the tree and runs all ~630
tests (asan: about 6.5 minutes of tests on 4 cores). After that build, run just
the tests you care about from the same tree, with the harness's options:

```sh
ctest --test-dir rust/tools/sanitize/out/build-asan -R '^test_rtmp_send_batch$' --output-on-failure
```

with `ASAN_OPTIONS=detect_leaks=1:fast_unwind_on_malloc=0` (see
`SANITIZER_OPTIONS` in `sanitize.py`). For valgrind, build with the
`valgrind` tool (no per-target ASan: `-DOBS_TEST_SANITIZE_PER_TARGET=OFF`) and
run the test binary under the `VALGRIND` argument list from `sanitize.py`
(`--error-exitcode=99 --leak-check=full --errors-for-leak-kinds=definite ...`).
Exit 99 is a valgrind error, 77 is a skip.

Run a timing-sensitive test alone and expect it to be slower than in Tier 2 (see
[timing-tests.md](timing-tests.md)).

## What valgrind costs

Measured in #3290 (4-core `ubuntu-26.04` runner): valgrind's unit tests took
53.8 minutes with 2 at a time, against 6.5 for asan on every core; serially
they did not fit the job's 150 minutes. The slowest (`test_slideshow_mem_cap`,
`test_output_filename_reserve`, `test_audio_clock_drift`) hit the 600 s
per-test limit. More than 2 valgrind jobs on 4 cores broke 9 tests on timing.
Run valgrind on a handful of tests, not the suite, on a local machine.

`stale_after`: the numbers change with the runner and the test set; re-measure
from the latest `sanitizers.yaml` run after that date.

---
type: concept
title: Running sanitizers locally
description: Asan or valgrind on a few tests through the harness on a local machine, and what valgrind costs per test.
sources: ["#3290", "#3238", "#962", "#3485", "#3366", "#3489"]
status: active
stale_after: 2027-05-01
---

# Running sanitizers locally

## The harness

`rust/tools/sanitize/sanitize.py` is what CI runs (`sanitizers.yaml`):

```sh
uv run --no-project rust/tools/sanitize/install_ubuntu_deps.py      # Ubuntu, once
uv run --no-project rust/tools/sanitize/sanitize.py asan      # or tsan / valgrind
uv run --no-project rust/tools/sanitize/triage.py rust/tools/sanitize/out/asan
```

It configures and builds OBS for the tool in `$SAN_OUT/build-<tool>` (default
`rust/tools/sanitize/out/`), runs every registered cmocka test under it, then
launches the app headless. Results go to `$SAN_OUT/<tool>`. `SANITIZE_JOBS`
sets how many tests run at once (`1` runs serially on one shared display);
`SKIP_TESTS=1` only relaunches the app.
`ZCCACHE_MAX_PARALLEL_COMPILES` sets how many files zccache compiles at once.
The harness does not set it: only `sanitizers.yaml` does, for the 4-core
runners, so a local machine with more cores keeps zccache's default or sets its
own value.

## A few tests

The harness has no test filter: one full run builds the tree and runs all ~670
tests (asan: about 5.5 minutes of tests on 4 cores). After that build, run just
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

`ctest -R` does not run a test the way the harness does. `run_one_test` in
`sanitize.py` gives each test its own Xvfb server and its own `*_OPTIONS` log
path, and the tests run side by side. In #3366 a test failed in the asan job and
passed in Tier 2. If `ctest -R` does not show a failure from the job, call
`run_one_test` from the built tree, then change one environment variable at a
time.

Run a timing-sensitive test alone and expect it to be slower than in Tier 2 (see
[timing-tests.md](timing-tests.md)).

## In a container

The harness does not know it runs in Docker. On a build machine:

- tsan needs `--security-opt seccomp=unconfined` on a recent kernel (7.0). Without
  it, tsan stops with "incompatible memory layout, unable to disable ASLR".
- With `--cpuset-cpus`, `nproc` follows the limit, but Python's `os.cpu_count()`
  does not, and that sets the test jobs. Set `SANITIZE_JOBS` too.
- `capture_hang` calls `sudo -n gdb` when the app does not exit. The linux-validate image has no
  `sudo`, so the harness stops with `FileNotFoundError: 'sudo'`. Install it, or
  relaunch with `SKIP_TESTS=1`.
- For a cold build, use a new `SAN_OUT` and `ZCCACHE_CACHE_DIR`.

## What valgrind costs

Measured in #3290 (4-core `ubuntu-26.04` runner): valgrind's unit tests took
53.8 minutes with 2 at a time, against 6.5 for asan on every core; serially
they did not fit the job's 150 minutes. More than 2 valgrind jobs on 4 cores
broke 9 tests on timing.

A few tests take most of that time. In the nightly run 41312715761, 524 of 666
tests took under 5 s (median 1.6 s), and the 9 tests of 60 s or more took 43 of
the 96 test minutes. Three of them hit the 600 s per-test limit:
`test_slideshow_mem_cap`, `test_output_filename_reserve` and
`test_audio_clock_drift`. So a handful of tests is cheap on a local machine, if
these are not among them.

`stale_after`: the numbers change with the runner and the test set; re-measure
from the latest `sanitizers.yaml` run after that date.

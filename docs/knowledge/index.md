---
type: index
title: obs-rust knowledge bundle
description: What agents and contributors learned about this codebase the hard way, one concept per file.
status: active
---

# Knowledge bundle

Lessons that cost someone hours to find and that the next worker would otherwise
find again. One markdown file per concept, in the core of the
[Open Knowledge Format](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
(OKF): YAML frontmatter with `type`, `title`, `description`, `sources`, `status`
and, where a decision can invalidate the finding, `stale_after`. Nothing beyond
that core. `./lint` fails when a file here has no frontmatter or no `type`,
`title` or `description` (`ci/knowledge_lint.py`). Decided in #3438 (from
discussion #3426); a normal PR review is enough to add a concept.

To add a concept, add its file and run `./lint --fix`, which writes the list below
from the frontmatter. Do not edit the list by hand. When two concept PRs collide
in the merge queue, rebase, run `./lint --fix` and push. Quote a `title` or
`description` that holds ` #`: YAML ends a plain value there.

**If it can be a check, the check is the rule.** A concept explains why a check
exists and links to it; it never replaces one. History that teaches nothing for
the next change stays in its issue or PR.

## When to write a concept

A lesson goes in its own PR, after the code PR has merged and the result is known.
That PR cannot eject the code from the merge queue, and it can quote what really
happened. Text that describes what the code PR changes, such as a new setting,
stays in the code PR.

## Correcting a concept

1. Each claim names what can recheck it: the code path, the check, a CI run or a
   command.
2. A correction changes the same file in a normal PR and adds its evidence to
   `sources`. It replaces the wrong text, so a file never contradicts itself.
3. A claim that is wholly wrong is removed, or the concept gets `status: retracted`
   with one line that says why and links the evidence.
4. A claim that was right but can change uses `stale_after` or `status`.

## Concepts

<!-- concepts: generated, `./lint --fix` writes this list -->
- [Which asan tests keep the slow allocation unwinder](asan-allocation-unwinder.md) — Distro FFmpeg has no frame pointers, so the fast unwinder stops at av_malloc; only tests that link FFmpeg keep the slow one, and a test the scan does not find stays slow.
- [Most asan builds start cold](asan-cold-builds.md) — Only a master run saves the asan compiler cache, and the merge queue does not run asan, so check whether a run restored a cache before you compare its build time.
- [An asan failure on your PR](asan-pr-failure.md) — What to do when the asan job fails a PR: fix the leak or the test, find out if the failure is yours, and why the known list is the last step and not the first.
- [Float results depend on the host compiler](host-compiler-float.md) — The CI runner's gcc contracts mul+add into an FMA and the Tier 2 image's gcc does not, so the same C source gives different float results.
- [Running sanitizers locally](local-sanitizers.md) — Asan or valgrind on a few tests through the harness on a local machine, and what valgrind costs per test.
- [A PipeWire loop stopped right after its start can hang](pipewire-stop-after-start.md) — pw_thread_loop_stop() can wait forever when the loop thread has not entered the loop yet (PipeWire 1.6.2). Wait for an event source's callback first (thread-loop.start-signal is ignored before PipeWire 0.3.77); a blocking pw_loop_invoke does not help.
- [Dispatch sanitizers on a separate ref](sanitizer-dispatch.md) — A `workflow_dispatch` of `sanitizers.yaml` on a PR branch cancels the run in progress there; dispatch on a separate ref.
- [The sanitizer known list](sanitizer-known-list.md) — The `bug`/`tool` kinds in `rust/tools/sanitize/triage.py`, how to add an entry, and why a `bug` entry that runs clean fails the job.
- [A source's destroy runs later, on another thread](shutdown-deferred-destroy.md) — obs_source_destroy queues its work on the destruction task thread, so obs_shutdown can free what a queued destroy still uses; wait in a loop until the queue is empty.
- [Free what OBS's destroy path frees](test-leaks.md) — A test that skips OBS's destroy call leaks what that call frees, and the asan job fails on that since #3290.
- [Timing tests wait for the result](timing-tests.md) — Wait for the frame or packet you check, not a frame count or a fixed sleep; when a test needs the `timing` resource lock.
<!-- /concepts -->

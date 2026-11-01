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
that core. `./lint` fails when a file here has no frontmatter, no `type` or
`description`, or is missing from this index (`ci/knowledge_lint.py`). Decided in
#3438 (from discussion #3426); a normal PR review is enough to add a concept.

**If it can be a check, the check is the rule.** A concept explains why a check
exists and links to it; it never replaces one. History that teaches nothing for
the next change stays in its issue or PR.

## Writing tests

- [Timing tests wait for the result](timing-tests.md) — wait for the frame or packet you check, not a frame count or a fixed sleep; when a test needs the `timing` resource lock.
- [Free what OBS's destroy path frees](test-leaks.md) — a test that skips OBS's destroy call leaks what it frees, and the asan job fails on that since #3290.

## Sanitizers

- [The sanitizer known list](sanitizer-known-list.md) — the `bug`/`flaky`/`tool` kinds in `rust/tools/sanitize/triage.py`, how to add an entry, and why a `bug` entry that runs clean fails the job.
- [Dispatch sanitizers on a separate ref](sanitizer-dispatch.md) — a `workflow_dispatch` of `sanitizers.yaml` on a PR branch cancels the run in progress there.
- [Running sanitizers locally](local-sanitizers.md) — asan or valgrind on a few tests through the harness, and what valgrind costs per test.

## Toolchains

- [Float results depend on the host compiler](host-compiler-float.md) — the CI runner's gcc contracts mul+add into an FMA, the Tier 2 image's gcc does not.

---
type: concept
title: The sanitizer known list
description: The `bug`/`flaky`/`tool` kinds in `rust/tools/sanitize/triage.py`, how to add an entry, and why a `bug` entry that runs clean fails the job.
sources: ["#3290", "#3238", "#3365", "#3368"]
status: active
---

# The sanitizer known list

## What it is

`KNOWN` in `rust/tools/sanitize/triage.py` maps `(tool, test)` to
`(kind, why)`. A listed test's exit and its sanitizer reports are shown under
"Known (not counted)" and do not fail the job. The kinds:

- `bug`: the test is right and OBS (or its build) is wrong. `why` names the
  open issue, e.g. `("asan", "test_media_alpha_network")` (#3365).
- `flaky`: the test is not solid yet; `why` names the issue that makes it
  solid, e.g. `("asan", "test_async_delay_audio")` (#3368). It may pass meanwhile.
- `tool`: a limitation of the sanitizer itself, no OBS issue (two valgrind entries).

## The trap

A `bug` entry whose test ran clean is reported under "Known bugs that ran clean:
drop the entry (counted)" and counts as a finding, so with
`--fail-on-findings` (always on for asan) the job fails. That is on purpose:
the fix for the bug must remove its entry in the same PR, or the list goes
stale and starts hiding the next regression. A clean `flaky` or `tool` entry
is only listed.

## How to add an entry

1. Open (or find) the issue that tracks the failure; never add an entry without one,
   except `tool`.
2. Pick the kind honestly: a timing failure is `flaky` (see
   [timing-tests.md](timing-tests.md)), a leak in OBS is `bug`, a leak in the
   test is not an entry at all: fix the test ([test-leaks.md](test-leaks.md)).
3. Add `("<tool>", "<test>"): ("<kind>", "<what happens> (#<issue>)")`.
4. When the issue is fixed, drop the entry in the fixing PR.

## Checks

- `triage.py --fail-on-findings`, passed for asan by `ci/sanitizers_step.py` since #3290.

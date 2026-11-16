---
type: concept
title: The sanitizer known list
description: The `bug`/`third-party`/`tool` kinds in `rust/tools/sanitize/triage.py`, the LeakSanitizer suppressions in `lsan.supp`, how to add an entry, and why a `bug` entry that runs clean fails the job.
sources: ["#3290", "#3238", "#3365", "#3368", "#3370"]
status: active
---

# The sanitizer known list

## What it is

`KNOWN` in `rust/tools/sanitize/triage.py` maps `(tool, test)` to
`(kind, why)`. A listed test's exit and its sanitizer reports are shown under
"Known (not counted)" and do not fail the job. The kinds:

- `bug`: the test is right and OBS (or its build) is wrong. `why` names the
  open issue.
- `third-party`: the failure is inside a library OBS uses, and no narrow
  suppression can match it. `why` names the issue that tracks the upstream fix.
- `tool`: a limitation of the sanitizer itself, no OBS issue (two valgrind entries).

## Suppress a library's own leak, don't list the test

A leak whose whole allocation stack is inside a library (no OBS frame) goes in
`rust/tools/sanitize/lsan.supp`, one `leak:` line that matches only that
stack. `sanitize.py` passes it as `LSAN_OPTIONS=suppressions=...` to every
test and the app. The test stays off the known list, so any other leak in it
still fails the job. Match one library entry point, never a whole module
(`libavcodec.so` would also hide an `AVPacket` OBS never frees). The first one is
FFmpeg 8.0.1's leak in `avformat_find_stream_info` on a VP9-alpha WebM that starts
between keyframes, which `test_media_alpha_network` hit (#3365); FFmpeg 9.0.2 does
not leak there. Take a line out when the library is updated past the fix.

## The trap

A `bug` or `third-party` entry whose test ran clean is reported under "Known bugs that ran clean:
drop the entry (counted)" and counts as a finding, so with
`--fail-on-findings` (always on for asan) the job fails. That is on purpose:
the fix for the bug must remove its entry in the same PR, or the list goes
stale and starts hiding the next regression. A clean `tool` entry is only listed.

The former `flaky` kind was retired after its last tracked failure was fixed
(#3368, #3370). An intermittent failure still needs its cause investigated;
it is not a separate exemption kind.

## How to add an entry

Read [asan-pr-failure.md](asan-pr-failure.md) first: fix the test if you can. The
goal for this list is zero entries.

1. Open (or find) the issue that tracks the failure; never add an entry without one,
   except `tool`.
2. Pick the kind honestly: an OBS bug is `bug`, a library bug no suppression can
   match is `third-party`, a sanitizer limitation is `tool`.
   Fix timing tests to wait for their result ([timing-tests.md](timing-tests.md));
   a leak in the test is not an entry at all: fix the test
   ([test-leaks.md](test-leaks.md)).
3. Add `("<tool>", "<test>"): ("<kind>", "<what happens> (#<issue>)")`.
4. When the issue is fixed, drop the entry in the fixing PR.

## Checks

- `triage.py --fail-on-findings`, passed for asan by `ci/sanitizers_step.py` since #3290.

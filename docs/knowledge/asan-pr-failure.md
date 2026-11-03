---
type: concept
title: An asan failure on your PR
description: "What to do when the asan job fails a PR: fix the leak or the test, find out if the failure is yours, and why the known list is the last step and not the first."
sources: ["#3290", "#3563", "#3575", "#3576", "#3577"]
status: active
---

# An asan failure on your PR

## The trap

The asan job fails a PR on any unit-test failure or sanitizer report (#3290). The
quick way to get a green run is to add the test to `KNOWN` in
`rust/tools/sanitize/triage.py`. The test still runs, but it can no longer fail
the job, so a real bug in it stays hidden. A test that can not fail tells nobody
anything. The goal for that list is zero entries.

## What to do instead

Do these in order:

1. Read the job's triage summary, then the test's log in the `sanitize-asan`
   artifact (`tests/<test>.log`).
2. A report that names your change is yours: fix it. A leak in a test goes in the
   test ([test-leaks.md](test-leaks.md)). A leak in OBS goes in OBS.
3. A test that fails on timing: make it wait for its result
   ([timing-tests.md](timing-tests.md)).
4. A failure with no link to your change: look for an issue on that test. If
   there is none, file one with the log, and rerun the job once
   (`gh run rerun --job <id>`). Do not rerun it again and again to get a pass.
5. The known list is the last step, for a test that keeps failing unrelated PRs
   while its issue finds the cause. Never list a test your PR adds or changes.

"Flaky" is not a cause. #3576 is on a virtual clock, so a slow runner should not
change its result, and #3577 is a hang. Either can be a real bug. An entry stays
until the issue finds the cause, and the fixing PR drops the entry.

## How it showed up

#3563 failed asan on two different tests in two attempts, then passed on a later
attempt with no change to its code. It touched neither test. One failure was a
race in the test (#3575). The other (#3576) and a hang on another PR (#3577) have
no known cause yet.

## Checks

- `ci/test_sanitize_triage.py` `KnownListTest`: each `bug` or `flaky` entry names an
  issue, and the number of entries matches `KNOWN_ENTRIES`. To add an entry you
  must raise that number, which shows in the diff for the reviewer. To drop one
  you must lower it, so the number can not go stale.
- The rules for each kind are in [sanitizer-known-list.md](sanitizer-known-list.md).

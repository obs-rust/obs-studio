---
type: concept
title: Most asan builds start cold
description: "Only a master run saves the asan compiler cache, and the merge queue does not run asan, so check whether a run restored a cache before you compare its build time."
sources: ["#3249", "#2923", "#3129", "#3457"]
status: active
stale_after: 2027-02-01
---

# Most asan builds start cold

## The trap

An asan build time means little until you know its cache state. On 4 cores, a cold
build took 15 to 27 minutes and a warm one about 4. Cold times alone vary by
10 minutes between runs, so one run shows little about a change.

## Why the cache is often missing

- Only a run on master saves it (`Save zccache` in
  `.github/workflows/sanitizers.yaml`), and on master only the nightly runs asan.
- A PR runs asan only if it changes a ratcheted target's compile input
  (`ci/conversion_ratchet.py`, `ci/ci_scope.py`, #2923). The merge queue does not
  run it for that reason.
- A PR or a dispatch restores by prefix, so it takes the newest nightly's cache if
  there is one.
- The cache can be gone hours after the nightly. On 2026-11-01 the 14:57 and 15:44
  UTC PR runs hit it (build 4.3 minutes). From 19:59 UTC on, every run logged
  `Cache not found`. The cause of that eviction is open (#3249).
- A hit is not always a warm build. Two runs on 2026-11-01 restored the same older
  key and took 13.8 and 33.8 minutes.

## What to do instead

- Before you quote a build time, read the `Restore zccache` step of the job:
  `Cache restored from key` or `Cache not found`.
- Compare the time between the log lines `== [asan] configure + build` and
  `== [asan] unit tests`, not the job's total.
- Quote several runs, and say how many. To compare two changes, run them side by
  side on the same machine.

## Why `stale_after`

This depends on the cache budget in `ci.toml`, the runners and the fix for the
eviction. Check it again after that date, or when #3249 closes.

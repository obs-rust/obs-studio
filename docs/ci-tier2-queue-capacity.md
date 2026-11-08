# Tier 2 runner capacity on the merge queue (2026-11-07)

The queue keeps both complete Linux Tier 2 backends, but schedules one runner
per backend on PRs and merge groups. Background/main/nightly/release runs keep
the six partitions. This reduces test-runner requests by 67% without removing
assertions, changing the aggregate gate, or extending the 60-minute deadline.

## Measurement

Fetched with `gh api` on 2026-11-07, using
`repos/obs-rust/obs-studio/actions/runs?event=merge_group&created=>=2026-11-06T16:20:00Z&per_page=100`
and each run's `/jobs?per_page=100` endpoint. The response contained 90 runs
(all master queue refs), from November 6 19:01:41Z through November 7 16:27:07Z.
No additional page was needed (90 total runs; at most 41 jobs per run).

The local CSV `docs/ci-evidence/2026-11-07-merge-group-jobs.csv` contains
1,056 non-skipped job records with timestamps and computed seconds. It stays
untracked under the repository's measurement-output policy (#2686). Skipped jobs consumed no runner. For allocated
jobs, queue seconds = `started_at - created_at`; runtime seconds =
`completed_at - started_at`. Never subtract workflow creation: that would
count dependency wait. Unassigned/pending/cancelled jobs have blank durations;
they are not zero-second successes. Collection is a non-atomic snapshot: run
metadata and job status can differ when a run finishes between API calls.
Summary below includes only allocated jobs with both timestamps, including
failed/cancelled jobs. It is descriptive, not a controlled benchmark.

| Job | Completed allocated jobs | Queue median / max (s) | Runtime median / max (s) |
|---|---:|---:|---:|
| CI Mode 🛂 | 89 | 90 / 882 | 24 / 29 |
| Event Data 🔎 | 88 | 85 / 1268 | 16 / 24 |
| Rust Tier 2 Linux 🐧 (build) | 53 | 57 / 1046 | 307 / 653 |
| Fast Gate ⚡ | 85 | 17 / 1089 | 313 / 482 |
| Rust Tier 2 Linux 🐧 (tests ON) | 48 | 125.5 / 1381 | 217 / 242 |
| Rust Tier 2 Linux 🐧 (tests ON, serial-1-of-2) | 48 | 55.5 / 1922 | 148 / 182 |
| Rust Tier 2 Linux 🐧 (tests OFF, serial-1-of-2) | 48 | 108 / 2093 | 149.5 / 168 |
| Rust Tier 2 Linux 🐧 (tests OFF, serial-2-of-2) | 48 | 183 / 1698 | 142.5 / 171 |
| Rust Tier 2 Linux 🐧 (tests ON, serial-2-of-2) | 48 | 173 / 1976 | 142 / 157 |
| Rust Tier 2 Linux 🐧 (tests OFF) | 48 | 91 / 1531 | 215 / 275 |
| CI Gate ✅ | 86 | 16 / 928 | 14 / 23 |
| Cache Janitor 🧹 | 81 | 5 / 1699 | 13 / 33 |
| Qt Frontend Linux 🐧 | 9 | 19 / 1037 | 205 / 308 |
| Cross Compile 🧰 (mac-arm64,win-x64) | 17 | 3 / 150 | 180 / 209 |
| Rust Cross Lint 🦀 (aarch64-apple-darwin) | 8 | 3 / 518 | 293 / 331 |
| Rust Cross Lint 🦀 (x86_64-pc-windows-msvc) | 8 | 3 / 663 | 324.5 / 356 |
| Rust Cross Lane Proof 🧾 | 2 | 38.5 / 75 | 4 / 5 |

### Coordinator's example: run 42386260200

All eight quoted queue/runtime pairs are independently confirmed, plus the
remaining Tier 2 partition:

| Job | Queue seconds | Runtime seconds |
|---|---:|---:|
| Tier 2 OFF serial-1-of-2 | 2093 | 158 |
| Tier 2 ON serial-2-of-2 | 1976 | 144 |
| Tier 2 OFF serial-2-of-2 | 1698 | 149 |
| Tier 2 OFF suite | 1531 | 235 |
| Tier 2 build | 417 | 293 |
| Fast Gate | 152 | 411 |
| Tier 2 ON suite | 48 | 223 |
| Tier 2 ON serial-1-of-2 | 215 | 139 |
| CI Gate | 177 | 11 |

The six test jobs are the dominant fan-out (six of seven Tier 2 runners).
This confirms severe recent runner waits, but **not** that every entry over
the whole day was close to ejection: many earlier jobs waited 2–5 seconds.
The aggregate maximums above also show other Linux orchestration jobs waiting.
Neither runtime nor fan-out alone guarantees a future hosted allocation time.

## Decision and expected effect

- **Choose compact execution, not omitted tests.** `ci/tier2_matrix.py` emits
  OFF/ON × `all` on PRs and merge groups; elsewhere it keeps OFF/ON × suite,
  serial-1-of-2, serial-2-of-2. An explicitly full caller also keeps partitions.
  Applying the compact layout on PRs both reduces the shared runner demand
  and exercises the same execution path before queue admission.
- `all` explicitly clears every CTest part/shard/CPU partition selector.
  The existing harness executes the entire registry and CTest still honors
  `RUN_SERIAL`. Both C reference (OFF) and hybrid Rust (ON), export parity,
  unchanged C assertions, Fast Gate and Rust differential tests remain required
  whenever selected. No tests are deferred: only the six-runner scheduling
  layout stays on background events. A full backend takes longer on its own
  runner; it avoids four allocations, setups and tree downloads per entry.
- **Before/after expectation:** six test allocations become two; including
  the unchanged build, seven Tier 2 allocations become three. At queue depth
  five, test allocations fall from 30 to 10 (35 to 15 including builds).
  Current partition runtimes in the example total 1048 runner-seconds;
  the longest partition takes 235 seconds. Complete execution can add serial
  runtime to that critical path. Savings in hosted wait cannot be predicted
  from the recorded queue delays; measure after rollout rather than promising
  a 35-minute reduction or a hard upper bound.
- **Do not add same-head attestation.** A PR head SHA does not identify the
  synthetic merge tree. Existing input-keyed lane proofs already cover safe
  reuse; exact-input and trusted-run checks remain unchanged.
- **Do not increase the timeout.** Long runner waits are the demonstrated
  bottleneck, and reducing four allocations per entry addresses the demand.
  The measured low-wait jobs do not justify declaring a larger merge budget.

## Gate and background proof

Ruleset 25065309 was read, never written: active, sole required context
`CI Gate ✅` (integration 15368), ALLGREEN/SQUASH, max build/merge entries 5/5,
check timeout 60 minutes. No ruleset payload or migration is needed.

The aggregate gate and `ci/gate_jobs.py` are unchanged. Real merge-group
[run 42382595386](https://github.com/obs-rust/obs-studio/actions/runs/42382595386)
shows Tier 2 ON job 128435760394 failing and `CI Gate ✅` job 128436702367
**failing**, both the direct needs check and the run-jobs check. This is real
failure propagation evidence for the preserved gate, not a synthetic passing
check. Any future change to that gate requires observing its new version fail.

All six background test partitions actually **ran and passed** on master push
[run 42383307878](https://github.com/obs-rust/obs-studio/actions/runs/42383307878):
OFF suite 128440506750, OFF serial-1 128440506732, OFF serial-2 128440506729;
ON suite 128440506686, ON serial-1 128440506722, ON serial-2 128440506662.
The same matrix is retained for push, schedule and full release callers;
regression tests exercise event selection. These historical runs prove current
execution, not post-merge execution of this unmerged proposal. The coordinator
should compare first post-merge main/nightly runs and queue waits before
claiming a measured speedup.

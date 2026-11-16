# Merge-group scope and native runner capacity — 2026-11-04

Current decision (#4747, authorized in #4743): merge groups execute selected
heavy checks on the merge result. This supersedes the November 4 Linux-only
placement decision described below. PR routing stays unchanged until the
separate admission cutover; full application builds remain nightly/manual/release.

The immutable event base/head diff includes every PR in the group. Core/shared
changes select both native Windows and macOS Rust tests and Tier 2; platform
paths retain their current selection. Linux Tier 2 runs two complete suites,
OFF and ON, from the existing shared build/runtime producers. Qt, C/C++ cross
compilation and touched Linux targets keep their path rules. Every non-docs
group also runs Windows MSVC test linking and clippy/Dylint for the host plus
all five cross targets. The two main cross targets still link tests with
`--no-run`; the extra three lint only. GNU-oracle link coverage is retained.

No client trailer, hosted input proof or cached test/lint verdict may suppress
queue execution. Build and Format set `SOLDR_VERDICTS=off`; compiler/dependency
artifacts may still be restored. CI Gate computes a full expected job manifest
independently of reusable outputs and requires success for every selected job,
including individual matrix legs and complete OFF/ON modes. Missing, skipped,
unfinished, cancelled and runnerless selected jobs reject admission. True
out-of-scope skips remain allowed. The required context is still `CI Gate ✅`.
Eligible Linux jobs use the selected `MQ_RUNNER_LABELS` with hosted fallback;
native jobs retain native runners. Capacity failures remain failures and never
authorize bypassing the queue. Cancellation and independent janitor behavior
are unchanged. Security scans, sanitizers and packaged/hardware qualification
remain on their existing nightly/manual/release events.

## Failure and cause

[Merge-group run 41859960760](https://github.com/obs-rust/obs-studio/actions/runs/41859960760)
concluded failure. Event Data job 126678747964 logged
`full=false core=true macos=true windows=true qt=false tier2=true asan=false`.
CI Gate job 126688014957 reported
`CI Gate did not receive the merge group's core scope`.
The caller consumed `needs.build-project.outputs.core` across a reusable-workflow
boundary. That value was empty after nested macOS jobs 126679116973 and
126679117143 ended cancelled, with empty runner names and no assigned runners
or steps. The gate checked scope before querying jobs and hid the cancellation.
The observed evidence establishes lost output despite successful computation;
GitHub's internal output propagation after cancellation is not observable.

The gate now reads `merge_group.base_sha` and `head_sha` from `GITHUB_EVENT_PATH`
and diffs those immutable commits. Invalid/missing fields, unavailable commits
and unsuccessful Event Data fail closed. Job failures are printed first, so a
missing scope input cannot hide them. A cancelled job with explicit empty
runner identity and no steps is diagnosed as a runner-capacity condition and
still fails. Other cancellations, failures, timeouts, startup failures and
action-required results continue to block. No automatic retry masks failures.
Maintenance exemptions are unchanged. No ruleset changes are needed: the sole
required context remains `CI Gate ✅`.

## Capacity evidence

[forge-ci/zccache run 41857782623](https://github.com/forge-ci/zccache/actions/runs/41857782623),
observer job 126671059827, concluded failure and observed macos-15 queued for
909 seconds. Another repository and runner image also exhibited starvation;
this supports hosted capacity pressure rather than an OBS code failure.

## Real master-push execution baseline

[Push run 41849221687](https://github.com/obs-rust/obs-studio/actions/runs/41849221687)
concluded success. macOS Rust Tests job 126640904675 ran 14:36:42–14:45:30 UTC
and macOS Tier 2 job 126640904805 ran 14:36:42–14:47:04 UTC on November 4,
both with assigned runners and success conclusions. Windows jobs 126640904718
and 126640904827 also ran successfully. The macOS application job was skipped,
as intended, and is not execution evidence. Post-change queue/push observations
belong on the PR. A cancelled workflow is never green proof.

## Regression coverage

RED commit `3b23668df`: three focused tests failed with seven assertion failures:
missing capacity diagnosis, a healthy queue rejected for empty reusable output,
and native queue selection. That change resolved scope independently and preserved fail-closed diagnostics.
The current tests restore selected heavy execution and enforce its manifest.
Tests extend the existing Python files. Rust/C/API port tiers do not apply to
this CI-only change. Push-scope regressions preserve native selection for core
and platform changes; tests do not substitute for real hosted execution evidence.

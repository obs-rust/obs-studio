# Merge-group scope and native runner capacity — 2026-11-04

Decision: merge-group execution is Linux-only. Native Rust tests and Tier 2
remain on master pushes for core/platform changes, and full native application
builds remain on nightly/manual/release events. Application builds already did
not run on merge-group or master push; none are newly deferred here.

This supersedes #3606. Native compiler/linker/runtime regressions can land
before detection and require fixing forward or reverting. Linux cross-lint
does not replace native runtime coverage. Native PR path selection is preserved. Changes to `ci/ci_scope.py` also select
native validation on master push, so routing changes exercise their consumers.
The queue continues to validate its selected Linux work.

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
and native queue selection. GREEN resolves scope independently, preserves
fail-closed diagnostics, and selects only Linux execution on merge-group.
Tests extend the existing Python files. Rust/C/API port tiers do not apply to
this CI-only change. Push-scope regressions preserve native selection for core
and platform changes; tests do not substitute for real hosted execution evidence.

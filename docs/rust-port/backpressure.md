# Back pressure and AI rebase

## What failed (2026-11-09)

By 14:00 UTC, 114 PRs were open and nothing had merged for more than 5 hours.
Every part of the tooling made its own admission decision from fixed knobs:

- Pool managers kept 8 workers busy, and each worker opened a PR when it finished.
- Chaperones (`pr_merge_watch --enqueue-when-ready`) re-enqueued every PR the queue ejected.
- Rebase waves force-pushed whole sibling sets at once, which restarted CI on every one of them.

None of those parts measured the one quantity that mattered: how fast the merge queue was actually landing PRs. Hosted runners stopped picking up jobs, so that rate fell to zero. Every PR that was still being added after that point increased queued runs and conflicts, which caused more rebases, which queued more runs again. A static cap (lesson 44's "about 15 PRs") would only have moved the cliff to a different spot. It is too low when runners are fast and too high when they are starved.

## Design: measure capacity instead of configuring it

`ci/backpressure.py` takes its numbers from live GitHub state and has no settings:

| signal | source |
|---|---|
| capacity | PRs merged in the last 2 h |
| width | `max_entries_to_build` from the repository's active merge-queue ruleset; fallback 5 if unreadable |
| in flight | union of queue entries and non-draft armed PRs, excluding only conclusively parked red PRs |
| starvation | age of the oldest queued job in its current run attempt (20 min or more counts as starved); job-less runs do not count |

When `capacity > 0`, the limit is `max(capacity, 2 * width)`. After a stall (`capacity == 0`), the limit is `width`. The gate is **open** when `in_flight < limit` and runners are not starved; otherwise it is **closed** (exit code 75). Starvation always forces headroom to zero, whatever the limit. Unreadable PR or queue state still fails closed.

The old one-PR-per-2-h ramp after a stall took about 2 h to recover, even though the queue tests `width` entries at once. Attested PRs reach the queue in about 66 s with no PR-side CI. Allowing one build group's width during a stall and at least two during normal operation keeps that capacity useful. The oldest queued job remains the catastrophic-failure guard from the 2026-11-09 collapse. Width is read once per invocation; an unreadable ruleset uses 5 without closing admission by itself. JSON verdicts report `width`, `width_source` (`ruleset:<id>` or `fallback`) and `limit_reason` (`normal`, `stall` or `starved`).

### Who obeys it

- `./push` opens a new PR as a **draft** while the gate is closed. The work is still published, but it uses no queue capacity.
- Pool managers check the gate before launching producers. When it is closed, they assign rebase and fix work instead of new ports.
- Chaperones enqueue only while it is open.
- `ci/ai_rebase.py --dirty` rebases at most the gate's `headroom` PRs per run, oldest first, because every rebase restarts CI.

## AI rebase

`ci/ai_rebase.py` replays a PR onto `origin/master`. At each stopped step it runs the clud agent unattended (`clud --openrouter --model ~deepseek/deepseek-flash-latest --unattended`) with the instructions in `ci/ai_rebase_prompt.md` and the list of conflicted files. The agent can read neighbouring code and run the repo's own fixers (`test_test_graph_sorted.py --fix`, `soldr cargo fmt`). It only stages files: the script continues the rebase and judges the result. Without clud, each file goes to the model directly. `AI_REBASE_MODEL` overrides the model.

A resolution is refused, and the PR is left untouched with a comment explaining why, if any of these holds:
- conflict markers remain,
- more than 2 lines that either side added are missing,
- `git diff --check` fails,
- the file is a lock file or binary.

The push uses `--force-with-lease` against the head that was fetched, so an author's concurrent push always wins. It then decides whether the PR goes back in the queue, biased towards yes: auto-merge is armed (pinned to the new head) unless the PR is a draft, says hold or "do not enqueue" in its body or labels, has changes requested, failed more than two real checks on its old head, or failed in the merge queue. Back pressure is left to the queue and the gate. The arm command leaves the merge strategy to the queue (which squashes); a rejected arm is reported as `failed to arm auto-merge` and makes the script exit nonzero even when the push succeeded.

### Cooperation with the coordinator, manager and worker pool

The bot owns mechanical rebases of DIRTY PRs and landing stacked drafts after their parent merges. The coordinator/manager assigns a worker to real failures or rebases the bot refuses, and checks whether the bot is enabled before assigning overlapping rebase or landing work. Both bulk selection and explicit PR runs respect these signals:

| label | owner and contract |
|---|---|
| `pool-active` | A pool worker adds it before working on an existing PR and removes it when retiring. The bot reports `skipped: worker active` and does not fetch, check out, rebase or push that PR. |
| `queue-failed` | The manager or requeue bot adds it after a real queue test/build failure; the manager dequeues and disarms the PR and stops its watcher. The rebase bot reports `left out: failed in the merge queue` and leaves it disarmed. Remove it only after a worker fixes and verifies the failure. Watchers must honor the label too. |

The bot also grants at least 30 minutes of quiet time after a head push, reporting `skipped: worker active`. GitHub often returns null for commit `pushedDate`, so the cheap signal is PR `updatedAt`: pushes advance it even when the commit was authored earlier. Comments or label changes can conservatively extend that quiet time. Missing activity data also skips the PR. Skipped workers consume no bulk headroom; the remaining candidates keep their original oldest-first order.

Even without `queue-failed`, the bot checks the most recent `removed_from_merge_queue` timeline event before rebasing. A `failed_checks` ejection on the current PR head prevents re-arming. The event's `beforeCommit` is a synthetic queue commit, so the bot uses ordered commit/force-push timeline events to bind the failure to the PR head, paging backwards when necessary. A newer head or a later non-failure removal does not inherit an old failure. Unreadable or incomplete failure history refuses the operation instead of guessing. The old head's verdict survives the bot's own rebase; rebasing alone cannot clear a real failure.

Stacked drafts keep the first line `Stacked on #P: review only the commits after <sha>.` Once P merges, the bot replays only commits after that SHA, replaces the stacking note and marks the PR ready, subject to the same ownership, holds and queue-failure guards. Lease pushes, FIFO ordering and back-pressure headroom remain in force. Bot pushes carry no client attestation, so the rebased PR runs fallback CI.

### Automatic infrastructure re-arm

`uv run --no-project --script ci/ai_rebase.py --requeue-ejected` runs before the rebase steps in the same workflow at :17 and :47 (also on master pushes and manual dispatch). It uses `GITHUB_TOKEN` with `contents: read` and `pull-requests: write`; Actions evidence is public read-only data. It needs neither `AI_REBASE_TOKEN` nor the model key, performs no checkout changes or pushes, and leaves merge strategy to the queue. Missing `AI_REBASE_TOKEN` disables only rebasing. Refs #4896.

The latest queue/head transition must be a removal within three hours, bound by preceding commit/force-push events to the unchanged PR head. The PR must be open, disarmed, out of draft, without a hold, `draft`, `pool-active` or `queue-failed` label or requested changes. The worker quiet period applies. Conflicts stay with the rebase path; manual removals are never retried. Immediately before mutation the bot refreshes the PR and queue transition.

- `checks_timed_out` is retried directly. Otherwise the bot reads the exact synthetic `beforeCommit`'s merge-group runs and their latest-attempt jobs, including failed jobs inside cancelled runs.
- Cancellation, runner-loss diagnostics, checkout/setup-soldr/cache-restore failures and explicit network errors qualify as infrastructure. Compiler/assertion failures take precedence over network text. A self-hosted failure also qualifies when the same workflow and exact matrix job passed on a different runner within one hour.
- Every substantive failed job must qualify. The downstream `CI Gate ✅` failure is ignored only with supporting failed/cancelled job evidence. Real test/build failures get `queue-failed` and remain disarmed; missing, unreadable or ambiguous evidence leaves the PR untouched.

One automatic attempt is allowed per PR per head SHA, across schedules and further ejections. Before `gh pr merge N --auto --match-head-commit SHA` (without `--squash`), the bot posts `<!-- ai-rebase:requeue-ejected:SHA -->` in a hidden comment. It pages through comments when checking this marker; restoring an identical SHA cannot reset the cap. The marker reserves the attempt before arming, so a rejected arm consumes the attempt and is reported as a failure. Workflow concurrency serializes these reservations. A run performs at most ten actions by default (`--limit`); `--dry-run` classifies without comments, labels or arming. Explicit PR numbers can narrow a manual script run.

`.github/workflows/ai-rebase.yaml` runs the script on every master push, every 30 minutes, and on manual dispatch with explicit PR numbers.

It needs two repository secrets:

- `OPENROUTER_API_KEY`.
- `AI_REBASE_TOKEN`: a GitHub App installation token or a fine-grained PAT, with **contents: write** and **pull-requests: write** on this repository. It cannot be `GITHUB_TOKEN`, for two reasons. A push made with `GITHUB_TOKEN` starts no workflows, so the rebased PR would never get CI. And the repository's maximum `GITHUB_TOKEN` grant is read-only.

Rebased commits keep their authors; the committer becomes the token's identity. Only same-repository PRs are rebased, never forks. A dry run on #4235 (2026-11-09) resolved 3 conflicted files in one step and kept all 455 added lines.

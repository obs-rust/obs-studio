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
| in flight | open non-draft PRs with auto-merge armed (queued or waiting for checks) |
| starvation | age of the oldest `queued` workflow run (20 min or more counts as starved) |

The gate is **open** when `in_flight < max(1, capacity)` and runners are not starved. Otherwise it is **closed** (exit code 75). By Little's law this holds the queue wait at about 2 h, whatever the runner speed is.

The limit is never below 1, so after a stall one PR can always probe the queue. When that PR merges, capacity rises and admission opens again (additive increase). When merges stop, the 2 h window drains, the limit falls and admission closes (multiplicative decrease). It needs no tuning and no human to reopen it.

### Who obeys it

- `./push` opens a new PR as a **draft** while the gate is closed. The work is still published, but it uses no queue capacity.
- Pool managers check the gate before launching producers. When it is closed, they assign rebase and fix work instead of new ports.
- Chaperones enqueue only while it is open.
- `ci/ai_rebase.py --dirty` rebases at most the gate's `headroom` PRs per run, oldest first, because every rebase restarts CI.

## AI rebase

`ci/ai_rebase.py` replays a PR onto `origin/master`. For each conflicted file it sends the file, with its conflict markers and the PR commit's subject, to OpenRouter. The default model is `~deepseek/deepseek-flash-latest`; `AI_REBASE_MODEL` overrides it. The model returns the whole resolved file.

A resolution is refused, and the PR is left untouched with a comment explaining why, if any of these holds:
- conflict markers remain,
- more than 2 lines that either side added are missing,
- `git diff --check` fails,
- the file is a lock file or binary.

The push uses `--force-with-lease` against the head that was fetched, so an author's concurrent push always wins. It then decides whether the PR goes back in the queue, biased towards yes: auto-merge is armed (pinned to the new head) unless the PR is a draft, says hold or "do not enqueue" in its body or labels, has changes requested, or failed more than two real checks on its old head. Back pressure is left to the queue and the gate.

`.github/workflows/ai-rebase.yaml` runs the script on every master push, every 30 minutes, and on manual dispatch with explicit PR numbers.

It needs two repository secrets:

- `OPENROUTER_API_KEY`.
- `AI_REBASE_TOKEN`: a GitHub App installation token or a fine-grained PAT, with **contents: write** and **pull-requests: write** on this repository. It cannot be `GITHUB_TOKEN`, for two reasons. A push made with `GITHUB_TOKEN` starts no workflows, so the rebased PR would never get CI. And the repository's maximum `GITHUB_TOKEN` grant is read-only.

Rebased commits keep their authors; the committer becomes the token's identity. Only same-repository PRs are rebased, never forks.

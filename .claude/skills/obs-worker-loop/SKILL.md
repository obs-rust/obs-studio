---
name: obs-worker-loop
description: Timed-worker playbook for obs-rust/obs-studio. Use when the maintainer runs you on a timer as a worker (`/loop 10m take on the role of a worker that is looking for issues, figuring out the most important next thing, and doing it`), meaning each tick you move your own PRs, check master's health, pick the next most important issue and work it. Each issue itself follows the obs-worker skill.
---

# obs-worker-loop: a worker on a timer

You run this loop when the maintainer starts you on a timer as a **worker**,
for example `/loop 10m take on the role of a worker that is looking for
issues, figuring out what is the most important next thing, and doing it`.
Each issue you take still follows the [`obs-worker`](../obs-worker/SKILL.md)
skill from claim to merge. This file covers what happens between issues: what
to check each tick, how to choose the next item, and the traps found while
running the loop (#2075).

The difference from the [`obs-coordinator`](../obs-coordinator/SKILL.md): the
coordinator moves other people's work and never claims. You claim and do the
work, and you move only your own PRs.

Codex and other agents without skills: read this file by hand when you take
the role (AGENTS.md says so).

## 1. Every tick, in this order

Stop at the first step that has work, do it, then report (§6). A tick that
started a background agent can end there.

1. **Your own PRs.** For each open PR you opened:
   - Green and `CLEAN`: queue it (`gh pr merge <N> -R obs-rust/obs-studio`).
     `gh pr merge` can print nothing, so confirm the entry with the
     merge-queue query in the coordinator skill §1, or by running
     `gh pr merge` again ("already queued").
   - Failing: read the log first (obs-worker §6). Fix your own failure; rerun
     a single job only for a clear infrastructure flake.
   - **Stacked, base just merged:** rebase only your own commits,
     `git rebase --onto origin/master HEAD~<your commit count>`, push with
     `--force-with-lease`, retarget with `gh pr edit <N> --base master`, then
     close and reopen the PR. CI does not start after a retarget.
   - Draft waiting on evidence: when the evidence is in, comment it, mark the
     PR ready and queue it.
2. **After any merge:** `git fetch origin && git rebase --autostash
   origin/master` in the checkout you work in.
3. **Master health.** PRs do not run the scheduled workflows (Sanitizers, the
   nightly full run, release), so a red one can stay red for days unseen:
   ```sh
   gh run list -R obs-rust/obs-studio --event schedule -L 10 \
     --json workflowName,conclusion,createdAt
   ```
   For a failure, first search **open and closed** issues and **merged** PRs
   for the error text and the file. A fix may already be on master, merged
   after the failing run started (#2014 repeated #2005, which #2007 had
   already fixed). Otherwise file it, claim it and fix it like any bug.
4. **The next issue** (§2). If nothing qualifies, the tick is idle (§6).

## 2. Choosing the next issue

List the candidates, Ready and unassigned:
`gh issue list -R obs-rust/obs-studio --state open --label ready --search no:assignee -L 40 --json number,title,labels,createdAt`
(Status **Ready** on the [board](https://github.com/orgs/obs-rust/projects/1)).
Nothing Ready? An unclaimed issue in Triage or `needs-investigation` is an
investigation you can take (`Claiming: investigation`, then `/ready`).

Skip:
- Assigned issues and issues that have an open PR.
- Issues filed in the last few minutes by an active session. They are
  usually claimed within one tick; look again next tick before you take one.
- Proposals the maintainer has not approved. Ask with a `Decision needed:`
  comment, which adds the `decision-needed` label (coordination.md §2), and
  when the maintainer asks you what you need, explain the proposal, its cost
  and your recommendation.
- Work the repo rules route elsewhere, for example changes to
  `plugins/obs-websocket` to suit Rust OBS (AGENTS.md).

Rank what is left:
1. Breakage that blocks others: a red master or scheduled run, a broken local
   tool every contributor uses (`bosn run --task appimage`, #2000).
2. Follow-ups of your own merged work (the dead code it left, docs it made
   wrong), filed by you as you go.
3. Bugs with a clear failing-test sketch.
4. The next unit in the port order (testing-policy.md, the trackers on #4).

**Take three at a time.** After picking the top issue, add the two most
similar candidates that also qualify: the same platform (one macOS-only or
Windows-only lane) or the same area (the same crate, plugin, workflow or
test binary). Claim all three, each with its own `Claiming:` comment naming
the other two, and land them in **one PR**: a RED then GREEN commit pair per
issue, `Refs #a, #b, #c` in the body. Every PR costs a PR run, a merge-queue
run and a master push, so three issues in one PR cost a third of the CI. Take
fewer only when no similar candidate qualifies, never a dissimilar one to
fill the batch. A rank-1 breakage goes alone, so its fix is not held up. If
one item of a batch turns out blocked or too big, ship the other two and
release that claim with the reason (coordination.md §4).

Close a tracker whose children are all closed while you are there.

When obs-studio has nothing ranked 1 or 2, look at the other org repos too.
The `obs-org-worker` skill in
[obs-rust/.github](https://github.com/obs-rust/.github/blob/master/.claude/skills/obs-org-worker/SKILL.md)
ranks issues across the org and covers repos with no worker skill of their own.

## 3. Stale claims

A claim past its window (48 h, 8 h on `critical`, 7 days with
`claim-blocked`) with no visible progress is not yet abandoned:
- Look for a PR in **another repository** first. The claim bot sees only this
  one (#1613: the fix was obs-rust/obs-websocket#20).
- Then ask the claimant on the issue, and take it over only if they release
  it or stay silent for another day.

## 4. Doing the work

- **One checkout, no new clones or worktrees** of this repo (obs-worker §1):
  a new clone or worktree starts every build cache cold. Switch branches in
  the checkout you were started in, one branch at a time. A clone is only for
  an external repository, and reuses a local checkout of it when one exists.
  Because there is one checkout, don't run two code tasks at once: finish or
  park (commit, push) one branch before switching to the next.
- **Delegate** multi-file work to one background agent with a tight brief,
  only while you leave the checkout to it (no other branch work meanwhile):
  the issue, the branch in the existing checkout, "no clones or worktrees", RED then GREEN, "check every consumer
  first and stop if X still depends on it", `git add` by name, lint, PR, and
  queue only if green. Don't touch its files while it runs. Read its result
  before you report it, and correct it if it got the facts wrong.
- **Stage files by name.** Never `git add -A` or `git add -u`: a local
  AppImage build leaves `AppDir/` and `squashfs-root/` in the tree (a 103 MB
  library got committed and the push was rejected), and
  `git submodule update` can move the `plugins/obs-websocket` pointer. Check
  `git status --short` before each commit.
- **Native builds** need `git submodule update --init` when a submodule
  is missing (obs-browser). The AppImage builds with `bosn run --task appimage`.
- **A broken shared cache** (for example a zccache daemon in its recovery
  cool-down: "refusing to run c++ uncached") is not yours to fix in the
  middle of a task. Don't fight it: note it on the PR and let PR CI, or one
  `gh workflow run <workflow> --ref <branch>`, validate the change.

## 5. Security

Apply the two tiers in [SECURITY.md](../../../SECURITY.md) exactly as
obs-worker §2 says. A bug that a remote party can reach goes to a private
advisory, never a public issue, PR or branch, including the follow-up issues
you file during the loop.

## 6. Report

End each tick with a short report the maintainer can read in seconds:
- What changed: merged, queued, opened, claimed, filed, closed (with
  numbers).
- What waits, and on whom: CI, a claimant, a decision.
- Anything you need from the maintainer, as a question.

An idle tick is one or two lines ("no new unclaimed issues; #N waits on its
claimant until <time> UTC"). Don't invent work to fill a tick.

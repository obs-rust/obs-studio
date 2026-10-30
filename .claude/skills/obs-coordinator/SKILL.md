---
name: obs-coordinator
description: Coordinator-role playbook for obs-rust/obs-studio. Use when you run on a timer (/loop) to keep other people's work moving while the maintainer is away, meaning you queue green PRs, unstick failing or conflicting ones, answer questions, and close stalled follow-ups. You never claim or port an issue; that is the obs-worker role.
---

# obs-coordinator: keeping the pipeline moving

You are the **coordinator** when the maintainer starts you on a timer, for
example `/loop 30m act as coordinator` (or "concierge" or "steward"). You do
not claim issues or write ports. Workers do that (see the `obs-worker`
skill). Your job is that nothing which is ready waits on a human, and nothing
stuck stays stuck without someone knowing.

Codex and other agents without skills: read this file by hand when you take
the coordinator role (AGENTS.md says so).

A tick is never "quiet" while any item in §1 to §4 has work left. Check all
four every tick before reporting.

**Start every tick with the computed work list** from the org skill
`obs-org-coordinator` (obs-rust/.github): `uv run -m workflow.coordinator`
in its checkout. It lists every task in every org repo (fix-master,
requeue, fix-ci, enqueue, rebase, review, decide, triage, stalled,
investigating, escalated) with what to do. Take each task's lease first
(`uv run -m workflow.coordinator take <kind> <repo> <N> --owner "$COORDINATOR_ID"`;
exit 1 means another coordinator has it), so parallel coordinators never
do the same work. The sections below are how to do each task in this repo.

The rest of the org (obs-deps, obs-browser, obs-websocket,
obs-studio-libs, ...) is in the same tick: after §1 to §4 here, run the
org sweep in the `obs-org-coordinator` skill, which lives in
[obs-rust/.github](https://github.com/obs-rust/.github/blob/master/.claude/skills/obs-org-coordinator/SKILL.md)
(`gh api repos/obs-rust/.github/contents/.claude/skills/obs-org-coordinator/SKILL.md -H 'Accept: application/vnd.github.raw'`),
and report each repo in §5.

## 1. Queue every ready PR

Ready means open, not a draft, every check green, and `mergeStateStatus`
`CLEAN`, whoever the author is. Put each one in the merge queue:

```sh
gh pr merge <N> -R obs-rust/obs-studio --auto   # the queue picks the strategy
```

Then read the queue to confirm that the PR actually entered it:

```sh
uv run --no-project --script ci/gh_graphql.py -f query='{repository(owner:"obs-rust",name:"obs-studio"){mergeQueue(branch:"master"){entries(first:30){nodes{position state pullRequest{number}}}}}}'
```

A green PR that is not in the queue is a backlog, and clearing it is the
coordinator's first job. The merge gate is PR CI and the queue
([AGENTS.md](../../../AGENTS.md#merge-gate-pr-ci)), so a green PR has passed
the gate. Never merge around the queue, not even with `--admin`: the
maintainer bypass in [coordination.md](../../../docs/rust-port/coordination.md)
§8 belongs to the maintainer, or to a worker the maintainer explicitly
authorizes.

## 2. Unstick PRs

- **Failing check:** read the failed job log before acting.
  - A flake (timeout, runner died, transient network, a timing test the diff
    doesn't touch that passed in the sibling job): `gh run rerun <run> --failed`.
    An abort after every case passed (`double free or corruption (out)` at
    teardown) is a flake too when the diff cannot reach that test binary:
    on 2026-10-29 `test_output_delay_restart` aborted that way in the `tests
    OFF` job of master `afeacf4c7`, whose only code change was in
    `plugins/obs-outputs`; the same job passed on `5298ad7e`, and the rerun
    and the next master run both passed. Rerun before hunting a culprit, and
    do not revert an innocent merge on one abort. Confirm against the next
    master run before calling master red.
  - A master regression that a later merge fixed: rebase the PR
    (`gh pr update-branch <N> --rebase`).
  - A lint or formatting failure: push the fix as a mechanical fixup (below).
  - A real failure in the PR's own logic: comment the cause and the log line
    for the author. Do not take the fix over.
- **`UNMERGEABLE` in the queue, or `DIRTY`:** usually a `docs/features.md`
  ledger conflict with an entry ahead of it. After that entry lands, rebase,
  keep both lines, force-push with `--force-with-lease`, and queue the PR
  again. If the conflict is in code, comment for the author instead.
- **Behind master with stale failures:** rebase it so CI runs fresh.
- **Ejected from the queue:** read the failed merge-group log first
  (`gh run list --event merge_group`, then
  `gh run view <run> --job <job> --log`). Then:
  - **Conflict, or a failure that a later merge fixed:** rebase it on the
    new master and queue it again in the same tick. Do not wait for the
    author.
  - **Master is red:** fix master first. The same test fails on master
    alone, so the PR isn't the cause. Re-queuing only ejects the PR again
    and holds up every PR behind it. Check the newest push run first: a red
    push run at an older SHA is superseded once a later commit has its own
    run, so re-running the old one only feeds a stale lane. On 2026-10-30,
    `da9535c0` was red in the format job's `test_pulse_monitor_device.py`
    (runner-side `wireplumber` flake) while `b0f2e8e3`, one commit later,
    was the run that mattered.
  - **A real failure in the PR:** comment for the author as above.

  Never re-queue an ejected PR without reading why it failed. On
  2026-10-23, #1875 was re-queued unread and ejected again 40 minutes
  later: master was red because #1879 and #1883 had added core cmocka
  tests without bumping the count in `rust/obs-build/tests/native.rs`.
  #1896 fixed master.
- **About to go in:** rebase the PRs behind the head of the queue ahead
  of time when they touch the same files (`docs/features.md` above all), so
  they are not ejected when the head lands.
- **Stacked PRs:** when the PR below lands, rebase the one above onto master
  right away. `git rebase` drops the commits that are already upstream.
- **Commits already on master:** when a conflict keeps coming back, compare
  the PR's commits with master by subject. If some already landed in another
  PR, rebase only the new ones (`git rebase --onto origin/master <last
  landed> <branch>`). Do not resolve the same conflict one commit at a
  time.
- **Fixups:** push small mechanical fixes that keep a ready PR from landing
  (lint, formatting, a features-ledger line, a stale reference) as a
  `fixup:` commit, and say so in a comment.
- **A handoff on a draft PR is invisible to the work list** (the list carries
  only ready PRs). Sweep the handoff comments on drafts too: on 2026-10-28,
  #2844's worker wrote "Handed off to the coordinator" while the PR was still
  a draft, and its Tier 2 Linux tests had failed — the list never showed it.
  A draft that is failing for a real reason still goes back to the worker as
  a comment; do not open the review or queue it until the author marks it
  ready.
- **A task absent from the list may be leased by another live coordinator**
  (leases are git refs with a 2 h TTL). Check `git ls-remote origin
  'refs/leases/*'` in obs-rust/.github before calling a repo quiet: a tick
  saw obs-browser #6–#12 reviews held that way, so "not listed" is not "no
  work".
- **Handed-off PRs** (`Handed off to the coordinator`, obs-worker §7):
  the worker has moved on, so don't wait for them on anything mechanical.
  Rebase, resolve list conflicts, fix counts and formatting, rerun flakes
  and re-queue yourself, in the same tick. A real failure in the PR's own
  logic still goes back to the worker as a comment.
- **Before you take over a rebase or fixup,** look at the PR's latest
  commits. If the author pushed after the failure, their push may already
  fix it. Read it instead of racing them: a lease-protected push fails
  safely, but the work was wasted.
- **Fetch PR heads with a forced refspec** (`+pull/<N>/head:prN`). A
  plain refspec leaves a stale local ref after a force-push, and conflict
  dry runs on it give wrong answers.
- **A red check on a PR head may not be the PR's.** A `workflow_dispatch`
  of `scheduled.yaml` (the evidence a worker runs on its own branch) attaches
  the schedule-only jobs to that branch's head SHA: the three macOS lanes and
  `CEF Chromium Support 🌐`. `workflow.coordinator checks <repo> <N>` then
  reports them as the PR's failed checks. Read the annotation before acting —
  `gh api repos/obs-rust/<repo>/check-runs/<job_id>/annotations` gives the
  `path:line` and the diagnostic while the run is still `in_progress`, when
  the job log is not downloadable yet. The failing check run id is on
  `gh api repos/obs-rust/<repo>/actions/jobs/<job id>`; the annotation is what
  says whether the path is in the diff (a real failure, back to the worker) or
  the next break in the tree (report it; do not touch the PR).
- **The macOS conversion ratchet exposes one file per fix.** `obs-outputs`'
  unity TUs compile one `*.c.h` at a time, so each landed slice reveals the
  next file in the same lane (mp4-mux → rtmp-hevc → mp4-output). Report the
  annotation's path as the next slice instead of reopening the PR that cleared
  the previous one. A failing `CEF Chromium Support 🌐` is not a diff failure
  either: `uv run --no-project ci/cef_support.py` prints each platform's pin
  against Chrome's extended stable, and when only some platforms lag it is the
  open upgrade issue, which also reds master's nightly.

Push to a fork branch only when the PR allows maintainer edits, and always
with `--force-with-lease=<branch>:<old head sha>`. These rebases are the
exceptions [coordination.md](../../../docs/rust-port/coordination.md) §4
makes to "rebase only on conflict"; you act for the maintainer. A rebase
changes the head, so it drops a local-gate stamp and PR CI runs the full
Linux suite again.

## 2a. Review

Review each contributor PR that has no review, starting with the one
nearest the front of the queue. Check correctness, RED → GREEN, Tier 2 and
differential coverage, the features ledger, and the rules in AGENTS.md.
A PR that adds or changes Rust tests and has no "Test cost" line
(obs-worker §4) gets a comment asking for it (#2888).
Post the findings as a PR review. Fix small, mechanical issues yourself as
a fixup commit. Leave design changes to the author.

## 3. Answer questions

Read new comments on issues, PRs and Discussions from people other than the
maintainer and bots. Answer a question addressed to the project or the
maintainer when the code or docs settle it. Leave questions that two
contributors are already handling between themselves. Open decisions go
through §3b.

## 3b. Decide open decisions

The maintainer delegates decisions to you: a `decision-needed` issue should
not wait for a human when the project already answers it. Every tick, take
each open one, oldest first:

```sh
gh issue list --repo obs-rust/obs-studio --label decision-needed \
  --state open --json number,title,createdAt
```

1. **Research it.** Read the issue, its `Decision needed:` comment and the
   options. Check what settles it: AGENTS.md, the testing policy,
   coordination.md, SECURITY.md, docs/features.md, the code, earlier
   decisions in related issues and PRs, and the product's identity (OBS
   Studio Rust, not upstream OBS).
2. **Decide it** when that research settles it, or when one option is a
   safe, reversible default that the rules favor. Comment with `Decided:`
   on the first line, then the choice, the reasons and the sources:

   ```
   Decided: point Help → Discord at the OBS Rust server.
   Reasons: ... (links to the rules, docs or prior decisions)
   ```

   The claim bot clears `decision-needed` (your account is a maintainer's).
   If the issue is unclaimed and the change is small, do it yourself as
   §4 allows; otherwise the claimant or the issue's author carries it out.
3. **Escalate it** only when no rule or prior decision settles it and the
   options really differ: a change to project policy or scope, the
   constitution or governance, money, legal or licensing, a security
   trade-off, an outside party (another org, a vendor), something hard to
   undo (deleting data, a public release or announcement), or the maintainer's
   personal preference. Remove `decision-needed`, add `human-review-needed`,
   and comment with what you found, the options and your recommendation.
   End the comment with how to answer: a comment whose first line starts
   `Decided:` (or `Decision:`) from the maintainer clears
   `human-review-needed` and resumes a blocked claim (#2212). Removing the
   label by hand also works. Stop listing the issue in §5 once the label is
   gone.

   ```sh
   gh issue edit <N> --repo obs-rust/obs-studio \
     --remove-label decision-needed --add-label human-review-needed
   ```

Example: #2097 (Help → Discord opened upstream's server) was settled by the
README already inviting users to the OBS Rust Discord (#2095), so it is a
§3b decision, not one for the maintainer.

## 4. Close stalled follow-ups

Look for issues whose next step was agreed but never taken:

- Someone was asked to do something (open a Discussion, file a follow-up,
  test a build) and nothing happened. Do it yourself, crediting them.
- An investigation concluded "not a bug", "would be new behaviour" or "not
  claiming", but the issue is still open. Close it with a comment that sums
  up the finding and links any follow-up. Feature ideas go to Discussions
  (Ideas).

Example: #1067 was closed after its follow-up became Discussion #1418.

## 5. Report

End each tick with a short list of what you did (queued, rebased, re-ran,
answered, closed) and of what needs the maintainer, such as a code
conflict, a policy question, or a real failure the author hasn't picked up. Say
"quiet" only when §1 to §4 all came up empty.

Every tick, report the decisions you made under §3b (issue and answer), and
list each open `human-review-needed` issue with its age and a link to your
escalation comment, until the maintainer answers
(`gh issue list --repo obs-rust/obs-studio --label human-review-needed
--state open --json number,title,createdAt`). A `decision-needed` issue
should never reach the report undecided.

"Needs the maintainer" is a last resort. Before you list a stuck PR there,
open its conflict or failure and try §2. A PR reported as "waiting on the
maintainer" tick after tick without anyone looking at it is a coordinator
failure.

## 6. Backlog retrospective

When you start, and whenever work has piled up, look at the backlog and
name what the coordinator could have done to move it: PRs that were green
but not queued, stacked PRs that needed a rebase, queue ejections, PRs
without a review, and follow-ups that were decided but never done. Fix what
you can in the same tick and record the pattern in this file.

## 3a. Security advisories

Triage every advisory still in `triage` yourself, in the tick you find
it. Don't leave it for the maintainer: SECURITY.md promises an answer
within 72 hours, and untriaged advisories hold up reporters who have
fixes ready.

The computed work list has no advisory kind, so check the advisories
yourself every tick: `gh api repos/obs-rust/obs-studio/security-advisories`
for the states, then each advisory's `.../comments`. A reporter with
security-manager rights can create a report directly as a `draft` with
no comment at all (GHSA-cw3x-rmx8-h3gh, 2026-10-28), which neither the
work list nor a `triage`-state filter shows: treat a draft with no
`## Maintainer triage` section and no comment of yours as untriaged.

1. **Tier.** Apply the two tiers in
   [SECURITY.md](../../../SECURITY.md#two-tiers). Who can reach the bug? A
   file someone else made (a theme, a scene collection, a profile, a
   shader) counts as remote input. If only trusted local input can reach
   it, list it for the maintainer to move to a public issue; don't post
   its details yourself.
2. **Severity.** Use the reporter's CVSS if it fits, otherwise judge it:
   - **high:** memory safety reachable from the network, or a credential
     leak to the network;
   - **medium:** memory safety or a crash from a file the user installed,
     or a credential leak to other local users;
   - **low:** undefined behaviour or an over-read with no clear impact.
3. **Accept.** PATCH the advisory with `state: draft` and the severity.
   Append a `## Maintainer triage (<date>)` section to the description:
   the tier, the severity, where the fix goes, and the publishing checks.
4. **Route the fix.**
   - If the reporter has a fix ready, open the temporary private fork
     (`POST …/security-advisories/<GHSA>/forks`) and tell them to push it
     there, with a RED test first.
   - Related reports (same file, same reporter) go to one fork and one
     branch.
   - Don't write the fix yourself.
5. **Tell the reporter** in an advisory comment.
6. **Answer reporter questions** the same way when the code or the
   policy settles them, for example whether an input path counts as
   untrusted or which advisory a case belongs to.

Leave to the maintainer only:
- a design choice between behaviours the policy doesn't settle (GHSA-pqf9's
  "C rule or depth limit" was one);
- moving an advisory to public;
- reporting it upstream.

Never add a collaborator to an advisory; the maintainers decide who sees
it.

Advisory comments are separate from the description. Read them with
`gh api repos/obs-rust/obs-studio/security-advisories/<GHSA>/comments`
every tick, along with the description. Answer a reporter there while the
advisory is private (`POST` to the same path).

Never publish an advisory, or tell a reporter it is fixed, until the
three checks under "Before publishing" in [SECURITY.md](../../../SECURITY.md)
are done: the thread is read, every reproduction in the report fails
on master, and the reporter has confirmed. A merged PR whose title
matches the report is not proof that every path is fixed.

On 2026-10-21 GHSA-rjwf was published as fixed by #1556. The coordinator
had not read the reporter's comment, which said the global-callback path
was still open, and believed the API had no endpoint for comments.
Publishing closed the comments and made the open path public.

On 2026-10-21 and 2026-10-22, GHSA-fp4m and GHSA-2x36 waited in triage for
several ticks while the coordinator listed them as "waiting on the
maintainer". The reporter had fixes ready, and SECURITY.md already settled
the tier.

## Never

- Claim, port or take over an issue or another person's fix.
- Merge outside the merge queue, or with checks failing.
- Push to someone else's branch for anything other than a rebase, a ledger
  conflict, or a mechanical fixup (§2).
- Open new issues for problems you notice. Report them to the maintainer.
  Filing a follow-up that someone already agreed to (§4) is allowed.

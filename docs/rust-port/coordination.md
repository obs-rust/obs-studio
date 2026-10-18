# Coordinating work: issues, claims and pull requests

Many contributors here are AI agents working in parallel, including the
maintainer's own automated runs. Every rule below exists because its absence has
already cost duplicate work: two PRs for the same issue (#115/#119,
#116/#118, #121/#123), five PRs superseded by a bot run porting units another
agent had announced, and a PR merged into a feature branch instead of
master (#68). Read [testing-policy.md](testing-policy.md) for *how* to port;
this document covers *who does what, and when*.

## 1. Before starting anything

1. Search for existing work:
   - open PRs: `gh pr list --repo obs-rust/obs-studio --search "<unit or topic>"`
   - the issue and its comments, including claim comments
   - the tracking issue's checklist (#4 lists the trackers)
2. If someone has claimed it and the claim is fresh (see 3), pick something
   else, or comment offering help. Do not open a parallel PR.
3. If there is no issue yet, file one first (section 5) and claim it.

## 2. Claiming

- Comment on the issue: `Claiming: <item>` followed by a short numbered plan
  (one PR per C file or unit) and when you expect to open the first PR.
- The claim bot (`.github/workflows/claim-bot.yaml`) then adds the `claimed`
  label and reacts 🚀 to your comment; you don't need label rights. It is
  silent unless it refuses: a claim on an issue someone else holds, a
  takeover of a claim that isn't stale, or a `/reassign` from a
  non-maintainer gets a reply that mentions you. Automated runs skip issues
  and checklist items that are claimed or have an open PR; a fresh
  `Claiming:` comment counts even if the label is missing.
- On a tracking issue, claim one checklist item, not the whole issue.
- Claim only what you are working on now. There is no per-account limit
  (several agents may share one account); stale claims expire (section 3).
- **Feature issues with open questions:** answer each open question in the
  claim comment under **Decisions:** (the default you will build and why),
  and order the PRs so the first one does not depend on an unresolved
  answer. A question with no safe default is posted as **Decision needed:**
  on the issue, and the work that depends on it waits for an answer.

## 3. Keeping or releasing a claim

- A claim goes stale after a period with no progress: no comment from the
  claimant on the issue, and no commit, comment or review on an open PR they
  opened that references it (a draft counts).

  | Issue | Stale after |
  |---|---|
  | normal | **48 hours** |
  | labeled `critical` (crash, data loss, broken release or CI-blocking; maintainers set it) | **8 hours** |

  The hourly sweep then removes the label, without a comment. Anyone may take
  a stale claim by commenting `Taking over stale claim from @x`; the bot
  accepts it (🚀) only if the claim is stale. Commits on the old draft PR stay
  available: carry them over with `Co-Authored-By` (section 6).
- Maintainers (owner, member, collaborator) can move any claim with
  `/reassign @user`. The bot leaves issues labeled `claim-locked` alone.
- To stop, comment `Releasing <item>` and link any partial branch; the bot
  removes the label (🚀). To hand the claim to someone instead of dropping
  it, name them in the same comment — `Releasing <item> — handing it to
  @user`, `Handing <item> over to @user`, `It's yours, @user` or
  `/handoff @user` — and the label stays, moved to `@user` (🚀), so nobody
  else can take the item in between.

## 4. Pull requests

- **Base is `master`.** A stacked PR starts its description with
  `> **Stacked:** builds on #N; the new commits are …` and is retargeted to
  master before merge. Check the base before asking for a merge.
- **One unit per PR.** Branch names: `port/<unit>`, `fix/<issue>-<topic>`,
  `test/<unit>`, `infra/<topic>`.
- **Commit order:** characterization test (if the unit had none), then RED
  (failing test or `todo!()` core), then GREEN. The PR description names the RED
  commit, how it fails (test names or counts), and the GREEN result.
- **Bug fixes need a failing test first.** If the bug needs live services
  (PipeWire, ScreenCaptureKit, a server), factor the decision into a small
  plain-C or Rust function and test that (as #109 did).
- **Rebase on origin/master when you start.** Before claiming, coding or
  reviewing, run `git fetch origin && git rebase --autostash origin/master`
  so the work starts from current master. Later master changes are handled
  by the merge queue (below).
- **PR CI is the merge gate.** Open the PR (a draft is fine) as soon as the
  RED test exists; a red CI run on a PR is expected. The local gate
  (AGENTS.md) is recommended, not required: a stamped head lets CI skip the
  Linux unit-test jobs, and without a stamp PR CI runs the full suite. Do not
  build fork or self-hosted CI to pre-validate PRs. Local errors from your
  environment (soldr permissions, disk, Docker, host packages):
  [troubleshooting.md](troubleshooting.md).
  For a focused CI/configuration bug fix, local RED evidence plus a separate
  GREEN commit is sufficient: push both together when ready rather than
  requiring a remote RED run. Record both commands/results in the PR. This
  exception changes push timing, not the RED → GREEN requirement or the
  required gate. Documentation-only updates do not need RED → GREEN tests.
- **Merge queue:** master merges go through GitHub's merge queue. Queue a
  green PR with `gh pr merge <N>` (or the "Merge when ready" button); the
  queue reruns PR CI on the merge result with current master, and the
  required `CI Gate ✅` check decides. You do not need to rebase by hand for
  unrelated master changes; rebase only when GitHub reports a conflict.
- **Submodules move forward only.** PR CI fails when a PR moves a submodule
  pointer (for example `plugins/obs-websocket`) to a commit that does not
  descend from master's pointer, which usually means a stale checkout. Run
  `git submodule update` before committing, or put `submodule:` in the PR
  title for a deliberate move back. Locally: `uv run ci/submodule_direction.py origin/master`.
- **Fork PRs:** Actions asks a maintainer to approve workflow runs only for a
  first-time contributor's first PR (repository setting "Require approval for
  first-time contributors"). Once a PR of yours has merged, later fork PRs
  start CI as soon as they are opened. On a first PR, a maintainer approves
  the run; ping one in the PR if it waits more than a few hours.
- **User-facing features need one real run.** A change to UI, protocol
  output or logging is not complete until the real app has run once with it
  (its Tier 4 test, or by hand), and the PR shows the observed output. #846
  closed with every test green while the light it added called llvmpipe
  green (#929). Without such a run, use `Refs #N`.
- **User-visible changes get a ledger line.** A PR that changes what a user
  sees or can do (behavior, a setting, UI, log output, a protocol field, a
  platform or package, or a fix for a bug a user can notice) adds or
  updates its line in [docs/features.md](../features.md) in the same PR.
  CI, tests, ports with no change in behavior and refactors do not (#1250).
- `Fixes #N` only when the PR completes the issue; otherwise `Refs #N`, and
  tick the tracking-issue checkbox when it merges.
- **Draft status:** mark the PR ready when it is ready. If you move a PR back
  to draft, say why in a comment.
- **Rebasing someone else's PR** (maintainers may): push with
  `--force-with-lease`, and name the replaced head SHA in a comment.

## 5. Filing issues

- **Suspected vulnerability:** not an issue. Don't file, claim or push it;
  report it privately as [SECURITY.md](../../SECURITY.md) says, and work on
  the private fork the maintainers open on the advisory. Current default
  rule: if untrusted input can reach the defect (network, websocket, media
  files or streams, browser pages, scene or profile files from others),
  report privately; if only a local user's own actions or a test can reach
  it, file a public bug; if unsure, report privately (a maintainer can move
  it to public).
- **Bug:** use the bug template. Title `bug: <component>: <symptom>` (or
  `[crash] <component>: …`). Include file:line, a short excerpt, the class
  (OOB read, UAF, overflow, …), how it is reached, a proposed fix and a
  failing-test sketch.
- **Upstream OBS bugs:** add `(upstream #N)` to the title and credit the reporter.
- **Port:** use the port template. Title
  `rust: Port <path>.c to <crate>`; state the phase, its position in the
  policy's port order, its dependencies and whether a characterization test
  exists.
- **Oracle differences** (a stand-in behaving unlike libobs): their own issue,
  fixed before any port relies on that oracle (#79, #84, #145).
- **Feature ideas and questions** are not issues: post ideas in
  [Discussions → Ideas](https://github.com/obs-rust/obs-studio/discussions/categories/ideas)
  and questions in Q&A. Issues are for bugs and claimable port or task work;
  an idea becomes an issue once it is agreed and someone can pick it up.

## 6. Collisions

- If two PRs address the same issue, the claimed one wins; without a claim,
  the earlier one wins. The other author closes theirs with a link and offers
  any useful pieces (tests especially). Maintainers carry those over with
  `Co-Authored-By`.
- Conflicting design choices between open PRs (for example U+FFFD substitution
  vs. rejecting a path) go to the tracking issue under a **Decision needed:**
  heading before either PR merges.

## 7. Closing

- When an issue closes, leave a comment: what fixed it, what is left, and the
  follow-up issue for the rest.
- A superseded PR is closed by its author (or a maintainer) with a link to the
  replacement.

## 8. Master branch protection (decisions for #139, #692 and #699)

`master` blocks force pushes and deletion, including for administrators.

By default, changes reach master through GitHub's merge queue (ruleset "master
merge queue (#692)", squash merges). Queue a green PR with
`gh pr merge <N>`. Agents and contributors do not use `gh pr merge --admin`
without explicit maintainer authorization under the bypass policy below,
and never push to master.
The queue reruns PR CI on the merge result with current master, and the one
required check is `CI Gate ✅` (`ci.yml`). That job always reports: it
passes when every required job passed or was skipped (out of scope, attested by
the local gate on the PR, or covered by a lane proof) and fails when a required job
failed or was cancelled.
The cache janitor is maintenance and is excluded from this gate; see
[the pre-work policy](../ci-tooling.md#pre-work-cache-maintenance-and-cancellation).

Why the queue and not rebase-and-merge per PR (#699): over a day of queue
runs (58 runs, 37 PRs), 57 runs were green and one PR was removed for a
failed check, so a queued PR goes through on the first try. The delay came
from 11 administrator merges that bypassed the queue: each moved master under
it and rebuilt every queued group (up to 5 runs for one PR, about 65 minutes
from queueing to merge against a 16-minute run). Requiring branches to be up
to date instead would make every agent rebase and rerun CI after every merge,
which gets slower with each agent added. So the queue is the normal path,
and lane proofs (docs/ci-tooling.md) let a rebuilt group skip the lanes whose
inputs did not change.

**Maintainer bypass.** The ruleset lets the repository admin role bypass it
for pull requests only (`gh pr merge <N> --admin --squash`; direct pushes
stay blocked). It is for a maintainer landing a small, low-risk PR (docs, a
bot or CI tweak) that should not wait for a CI run. It skips CI, and it moves
master under every queued group, which then rebuilds. So bypass only when
the queue is empty (`gh pr list --search "is:queued"` shows nothing);
otherwise queue the PR and, if it must go first, jump it to the front,
keeping CI (#930 E2):
`gh api graphql -f query='mutation{enqueuePullRequest(input:{pullRequestId:"<node id>",jump:true}){mergeQueueEntry{position}}}'`
(the PR's own `CI Gate ✅` must already be green; GitHub refuses a PR whose
checks are pending or failing).

Explicit maintainer instructions to admin-merge authorize an agent to use
this exception; do not ask for the same authorization again. Recheck the
queue immediately before merging. If it has entries, use the queue and
jump-to-front path above to preserve their work. With an empty queue, a
small docs or CI fix may use the authorized bypass. Run focused checks
first; for a change to the workflow graph, wait for its PR `CI Gate ✅` to
pass before bypassing the redundant merge-group run. Record the
authorization, queue state and validation in the PR. Authorization for one
landing does not grant a standing bypass for unrelated work.

Each queue run's `CI Gate ✅` job summary shows its wall time, runner time
and attempt number (`ci/queue_report.py`); an attempt 2 or later is a
rebuild. If rebuilds after bypasses show up there, tighten this rule.

In an emergency an administrator may set the ruleset's enforcement to
"disabled", land the fix, and enable it again, saying so in the PR.

Do not require linear history while the project uses merge commits for PRs.
Review this policy again if direct pushes are retired or the gate's CI
reporting changes. Every PR still targets `master` as section 4 requires;
GitHub branch protection cannot prevent an author from choosing another
base, so check the base before merging.

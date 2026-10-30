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

The org-wide rules, commands and the definition of Ready are in
[WORKFLOW.md](https://github.com/obs-rust/.github/blob/master/WORKFLOW.md); the claim bot is the org's reusable workflow
(`.github/workflows/claim-bot.yaml` calls it). In short:

- Only an issue labeled `ready` can be claimed for implementation (`P0`
  issues never wait). An underdefined one is researched first: comment
  `Claiming: investigation`, then `/ready` once it meets the definition of
  Ready. Maintainers (and the coordinator) triage new issues with `/ready` or
  `/investigate`.
- Comment on the issue: `Claiming: <item>` followed by a short numbered plan
  (one unit per issue, one to three similar issues per PR, a big one alone; a call-site-wide warning class is one unit in any number of
  directories, see section 4) and when you expect to open the first PR.
- The claim bot then **assigns you** and reacts 🚀 to your comment; you don't
  need any rights. It replies only to refuse: an issue that is not Ready, is
  blocked by an open issue, waits on a decision or is claimed by someone
  else, a takeover of a claim that isn't stale, or a `/reassign` from a
  non-maintainer. Automated runs skip issues that are assigned or have an
  open PR.
- On a tracking issue, claim one checklist item, not the whole issue.
- Claim only what you are working on now. There is no per-account limit
  (several agents may share one account); stale claims expire (section 3).
- **Feature issues with open questions:** answer each open question in the
  claim comment under **Decisions:** (the default you will build and why),
  and order the PRs so the first one does not depend on an unresolved
  answer. A question with no safe default is posted as **Decision needed:**
  on the issue, and the work that depends on it waits for an answer. Use it
  only for that; a question with a safe default goes under **Decisions:**.
- **Open decisions:** a line that starts with `Decision needed:` (bold or a
  heading marker before it is fine) makes the claim bot add the
  `decision-needed` label (🚀, silent), from anyone. From the claimant it
  also counts as `Blocked:` (section 3). Only a maintainer clears it, with a
  `Decided:` or `Decision:` comment (first line) or by removing the label;
  nobody else's progress and no sweep removes it. That answer also clears
  `human-review-needed`, and on a blocked claim it removes `claim-blocked`
  and mentions the claimant with the decision quoted, so the work resumes
  (#2212). The coordinator (obs-coordinator skill
  §3b) researches each open decision every tick: it answers with `Decided:`
  when the rules, docs or earlier decisions settle it, and otherwise moves
  the issue to `human-review-needed`, the list that waits on the human
  maintainer. The list of open decisions is
  [issues labeled `decision-needed`](https://github.com/obs-rust/obs-studio/issues?q=is%3Aopen+label%3Adecision-needed+sort%3Acreated-asc),
  oldest first. Non-public work never posts `Decision needed:` on a public
  issue; use a general `Blocked:` reason.

## 3. Keeping or releasing a claim

- A claim goes stale after a period with no progress: no comment from the
  claimant on the issue, and no commit, comment or review on an open PR they
  opened that references it (a draft counts).

  | Issue | Stale after |
  |---|---|
  | normal | **24 hours** |
  | labeled `P0` or `critical` (crash, data loss, broken release or CI-blocking; maintainers set it) | **8 hours** |
  | labeled `claim-blocked` (the claimant set it; see below) | **7 days** after the last `Blocked:` comment |

  The hourly sweep then unassigns the claimant, without a comment. Anyone may take
  a stale claim by commenting `Taking over stale claim from @x`; the bot
  accepts it (🚀) only if the claim is stale. Commits on the old draft PR stay
  available: carry them over with `Co-Authored-By` (section 6).
- When the work waits on something you don't control (a maintainer
  decision, another PR, coordinated non-public work), comment
  `Blocked: <reason>` (or `/blocked <reason>`). Only the claimant can; the bot
  adds `claim-blocked` (🚀) and the claim lasts 7 days from that comment. A
  new `Blocked:` comment renews it; a takeover in the meantime is refused.
  A claimant's `Decision needed:` comment (section 2) blocks and renews the
  same way.
  Keep the reason general ("waiting on a maintainer", "waiting on #N",
  "waiting on non-public work"). Any other progress (an issue comment, a
  commit, comment or review on your PR) removes the label and the normal
  timer applies again, as do a release, handoff or `/reassign`. When the
  7 days end without a renewal the sweep removes both labels, silently.
- Maintainers (owner, member, collaborator) can move any claim with
  `/reassign @user`. The bot leaves issues labeled `claim-locked` alone;
  `claim-locked` wins over `claim-blocked`.
- To stop, comment `Releasing <item>` and link any partial branch; the bot
  unassigns you (🚀). To hand the claim to someone instead of dropping
  it, name them in the same comment — `Releasing <item> — handing it to
  @user`, `Handing <item> over to @user`, `It's yours, @user` or
  `/handoff @user` — and the bot assigns `@user` instead (🚀), so nobody
  else can take the item in between.

## 4. Pull requests

- **Base is `master`.** A stacked PR starts its description with
  `> **Stacked:** builds on #N; the new commits are …` and is retargeted to
  master before merge. Check the base before asking for a merge.
- **One unit per PR.** Branch names: `port/<unit>`, `fix/<issue>-<topic>`,
  `test/<unit>`, `infra/<topic>`.
- **Batch one to three similar issues into one PR.** A worker takes up to
  three issues that share a platform or an area (crate, plugin, workflow,
  test binary) and lands them as one PR, a RED then GREEN commit pair per
  issue, `Refs` naming each. Each issue stays one unit for review and
  revert; the batch only shares the PR, queue and master-push runs, so it
  costs a third of the CI. A big issue (a large port or subsystem such as
  the CEF work, `size:L` or larger) and breakage that blocks others go
  alone. See the `obs-worker-loop` skill §2.
- **Mechanical warning work is one PR per warning class, in any number of
  directories.** A call-site-wide compiler warning (#516) is a single class:
  fix it in one PR, with the flag that surfaces it removed in the same change,
  and put a findings table grouped by directory in the description so the diff
  can still be reviewed in slices. A per-directory series is not a rule that
  appears anywhere else in these docs, it costs a full `Pull` run (~35 min)
  plus a merge-queue run per directory, and every flag-list edit invalidates
  the compile cache (#2689). Measure the class with
  `uv run ci/warning_findings.py --compdb <tree compdb> --drop <the -Wno-* pin>`:
  it replays only the affected translation units with `-fsyntax-only` and
  prints the findings grouped by directory, which is both the RED evidence and
  the inner loop. The default `--language c` keeps its C units; add
  `--language cxx` (or `all`) for a C++ class such as the frontend's, whose
  units the C-only selector filters out. The limits do not move — explicit
  casts or widened types
  only, no behaviour change, no per-file `-Wno-*`, never an existing C test or
  public header, and the pin goes away in the same PR as the fixes.
  **A PR lane's tree is not the whole set.** Many plugin and platform sources
  compile only in the app builds (#2678, #2710), so a pin-drop that is clean on
  a Tier 2 tree can still fail a `Scheduled` run. Enumerate the app's C units
  from its own build tree — configure only, seconds, no compile:
  `uv run build-aux/appimage/build.py --steps version,configure`, then
  `ninja -t compdb` in `build_ubuntu/` (or the app build's volume) and replay
  that compdb. Vendored trees the tree exempts with a target-scoped `-Wno-*`
  (`librtmp`, `libcaption`) report nothing while their pin is in the flag
  list; re-run with `--drop` on it to check them.
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
  The coordinator (obs-coordinator skill, §2) acts for the maintainer and
  also rebases a PR that was ejected from the queue, a stacked PR whose base
  landed, a PR about to go in behind the queue head that touches the same
  files, and a `docs/features.md` ledger conflict. A rebase changes the head,
  so it drops a local-gate stamp and PR CI runs the full Linux suite again.
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
- **Rebasing someone else's PR** (maintainers, and the coordinator acting
  for them, may, for the rebases above and for mechanical fixups such as
  lint or formatting): push with `--force-with-lease`, and name the replaced
  head SHA in a comment.

## 5. Filing issues

- **Security bugs:** apply the tiers in
  [SECURITY.md](../../SECURITY.md) ("Two tiers"). A high-risk one (a remote
  party can reach it, or it leaks credentials) is not an issue: don't file,
  claim or push it; report it privately and work on the private fork the
  maintainers open on the advisory. If unsure, report privately. A low-risk
  one is a normal public bug and may carry the `security` label.
- **Bug:** use the bug template. Title `bug: <component>: <symptom>`; for a
  security bug, name the class in the symptom (`bug: obs-ffmpeg:
  use-after-free on seek`). Include file:line, a short excerpt, the class
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
merge queue (#692)", whose merge method is squash: each PR lands as one
commit). Queue a green PR with `gh pr merge <N>`. Workers and contributors
do not use `gh pr merge --admin` without explicit maintainer authorization
under the bypass policy below, and never push to master. The coordinator
never merges outside the queue, with `--admin` or otherwise.
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

**Maintainer bypass.** It belongs to the maintainer, or to a worker the
maintainer explicitly authorizes; never to the coordinator. The ruleset lets the repository admin role bypass it
for pull requests only (`gh pr merge <N> --admin --squash`; direct pushes
stay blocked). It is for a maintainer landing a small, low-risk PR (docs, a
bot or CI tweak) that should not wait for a CI run. It skips CI, and it moves
master under every queued group, which then rebuilds. So bypass only when
the queue is empty (`gh pr list --search "is:queued"` shows nothing);
otherwise queue the PR and, if it must go first, jump it to the front,
keeping CI (#930 E2):
`uv run --no-project --script ci/gh_graphql.py -f query='mutation{enqueuePullRequest(input:{pullRequestId:"<node id>",jump:true}){mergeQueueEntry{position}}}'`
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

The queue squash-merges, but the repository still allows merge commits
(older history and an admin bypass may use them), so do not require linear
history.
Review this policy again if direct pushes are retired or the gate's CI
reporting changes. Every PR still targets `master` as section 4 requires;
GitHub branch protection cannot prevent an author from choosing another
base, so check the base before merging.

# Coordinating work: issues, claims and pull requests

Many contributors here are AI agents working in parallel, alongside the
maintainer's grind bot. Every rule below exists because its absence has
already cost duplicate work: two PRs for the same issue (#115/#119,
#116/#118, #121/#123), five PRs superseded by a bot run porting units another
agent had announced, and a PR merged into a `grind/*` branch instead of
master (#68). Read [testing-policy.md](testing-policy.md) for *how* to port;
this document covers *who does what, and when*.

## 1. Before starting anything

1. Search for existing work:
   - open PRs: `gh pr list --repo obs-rust/obs-studio --search "<unit or topic>"`
   - the issue and its comments, including claim comments
   - the tracking issue's checklist (#4 lists the trackers) and the latest
     `grind:v1` status comment on #4
2. If someone has claimed it and the claim is fresh (see 3), pick something
   else, or comment offering help. Do not open a parallel PR.
3. If there is no issue yet, file one first (section 5) and claim it.

## 2. Claiming

- Comment on the issue: `Claiming: <item>` followed by a short numbered plan
  (one PR per C file or unit) and when you expect to open the first PR.
- Add the `claimed` label (or ask a maintainer to). The grind bot skips issues
  and checklist items that are claimed or have an open PR.
- On a tracking issue, claim one checklist item, not the whole issue.
- Keep at most two claims open at a time.

## 3. Keeping or releasing a claim

- A claim expires after **48 hours** without a PR or a progress comment. Anyone
  may then take the item after commenting `Taking over stale claim from @x`.
- To stop, comment `Releasing <item>`, link any partial branch, and remove the
  `claimed` label.

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
- **PR CI is the merge gate.** Open the PR (a draft is fine) as soon as the
  RED test exists; a red CI run on a PR is expected. The local gate
  (AGENTS.md) is recommended, not required: a stamped head lets CI skip the
  Linux unit-test jobs, and without a stamp PR CI runs the full suite. Do not
  build fork or self-hosted CI to pre-validate PRs.
- **Merge queue:** master merges go through GitHub's merge queue. Queue a
  green PR with `gh pr merge <N>` (or the "Merge when ready" button); the
  queue reruns PR CI on the merge result with current master, and the
  required `CI Gate ✅` check decides. You do not need to rebase by hand for
  unrelated master changes; rebase only when GitHub reports a conflict.
- **Fork PRs:** Actions asks a maintainer to approve workflow runs only for a
  first-time contributor's first PR (repository setting "Require approval for
  first-time contributors"). Once a PR of yours has merged, later fork PRs
  start CI as soon as they are opened. On a first PR, a maintainer approves
  the run; ping one in the PR if it waits more than a few hours.
- `Fixes #N` only when the PR completes the issue; otherwise `Refs #N`, and
  tick the tracking-issue checkbox when it merges.
- **Draft status:** mark the PR ready when it is ready. If you move a PR back
  to draft, say why in a comment.
- **Rebasing someone else's PR** (maintainers may): push with
  `--force-with-lease`, and name the replaced head SHA in a comment.

## 5. Filing issues

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

## 8. Master branch protection (decision for #139)

`master` blocks force pushes and deletion, including for administrators. It
does not require pull requests or status checks, and it allows merge commits.
The maintainer's direct-push tooling can therefore continue to publish a
locally tested, attested commit. Contributors should still use the claim and
PR workflow above; this setting is not a waiver of those rules.

Do not require the Linux Rust or Tier 2 CI jobs while the local-gate
attestation legitimately skips them. A skipped job is not a successful
required check. Revisit required checks only after CI exposes a stable,
always-reported aggregate result for both attested and full runs. Do not
enable a merge queue before that result exists; queued runs would need the
same gate and would delay the current grind and maintainer flows.

Do not require linear history while the project uses merge commits for PRs.
Review this policy again if direct pushes are retired or the gate's CI
reporting changes. Every PR still targets `master` as section 4 requires;
GitHub branch protection cannot prevent an author from choosing a `grind/*`
base, so check the base before merging.

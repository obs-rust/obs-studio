---
name: obs-worker
description: Worker-role playbook for obs-rust/obs-studio. Use whenever you act as a worker in this repo, meaning you pick up, claim, port, fix or test an issue, or build, lint, push or open a PR. It walks you through the repo's CI and contribution docs, from rebase to merge queue.
---

# obs-worker: picking up and landing one issue

You are a **worker** when you take one issue (port, bug fix, feature, test
or infra task) from claim to merged PR. This skill is the checklist. The documents it
links are the source of truth; when this file and a document disagree, the
document wins, and you fix this file in the same PR.

Codex and other agents without skills: read this file by hand when you take
the worker role (AGENTS.md says so).

## 0. Read the rules (every new session)

Read these before you start. Skim them again if they changed on master since
you last read them (`git log -1 --format=%cs -- <file>`).

1. [AGENTS.md](../../../AGENTS.md): quick rules (soldr, uv, platform
   boundary, naming, no edits to C tests or public headers).
2. [docs/rust-port/coordination.md](../../../docs/rust-port/coordination.md):
   claims, PRs, merge queue, collisions.
3. [docs/rust-port/testing-policy.md](../../../docs/rust-port/testing-policy.md):
   the four tiers and the per-unit workflow.
4. [docs/ci-tooling.md](../../../docs/ci-tooling.md): soldr, bosn, local gate,
   attestation, what to do when CI fails.
5. [docs/rust-port/troubleshooting.md](../../../docs/rust-port/troubleshooting.md):
   only when your environment errors.
6. [SECURITY.md](../../../SECURITY.md): low-risk security bugs are
   normal public bugs; high-risk ones (reachable by a remote party, or
   leaking credentials) are reported privately.

## 1. Start from current master

```sh
git fetch origin && git rebase --autostash origin/master
```

Work in your own branch, named `port/<unit>`, `fix/<issue>-<topic>`,
`test/<unit>` or `infra/<topic>`, in the checkout you were started in. Don't
touch other people's branches, and don't delete local branches you didn't
create.

**No new clones or worktrees of this repo.** Switch branches in the existing
checkout (`git switch -c <branch> origin/master`; stash or commit first).
Every build cache (zccache, the cargo target directory, CMake build trees,
bosn volumes) is keyed to a checkout's path, so a fresh clone or worktree
starts cold: it recompiles everything and fills the disk. A clone is allowed
only for a **different repository** (an external repo such as
obs-rust/obs-websocket or a private advisory fork), and even then first reuse
a local checkout of that repo if one exists (look under
`~/dev/obs-rust/obs-studio2-extern/` and `~/dev/`), fetching and branching in
it, so its caches stay warm. Clone only when no local copy exists.

**Disk hygiene — your checkout is one of four warm slots.** Each of
`~/dev/obs-rust/obs-studio{2..5}` carries a 10–30 GB `target/` and the box runs
four workers every 15 minutes, so keep your cache warm but do not let debris
pile up on top of it:

- When you finish a task, run `soldr gc target --root "$PWD"` (report-only; it
  lists every `target/` under your checkout with sizes). Purge the ones under
  **experiment subdirectories** — `build_ci_audit/soldr*`,
  `build_ci_audit/native-fp-*`, `build_ci_audit/deep-test-cache-probe-*`,
  `build_ubuntu/corrosion/*` — with
  `soldr gc target --root <that subdir> --purge --yes`. Those are cold one-shot
  probes nobody rebuilds (measured 4.2 GB across the fleet on 2026-10-28).
- Delete one-off CMake `build_*` trees you created for a diagnosis once they are
  >24 h old and nothing is building in them (no `/proc/*/cwd` inside; no
  `cmake`/`ninja`/`cc1` running). Always keep the newest `build_*`.
- **Never** purge your checkout's top-level `target/`, never touch another
  slot's caches, and never `docker system prune` — a cold rebuild of this repo
  is a 30–60 minute tax on whichever worker runs next.
- If your checkout exceeds ~25 GB, say so in your final message; the
  coordinator decides whether a slot retires. Do not purge to hide it.

## 2. Choose and claim one issue

1. List Ready, unclaimed candidates:
   `gh issue list --repo obs-rust/obs-studio --state open --label ready --search no:assignee`
   (or Status **Ready** on the [board](https://github.com/orgs/obs-rust/projects/1)),
   highest priority (`P0`..`P3`) first. Skip anything with an open PR
   (`gh pr list --repo obs-rust/obs-studio --search "<topic>"`). Nothing
   Ready you can do? Take an investigation: `Claiming: investigation` on an
   unclaimed issue in Triage or `needs-investigation`, then `/ready` it when
   it meets the [definition of Ready](https://github.com/obs-rust/.github/blob/master/WORKFLOW.md#definition-of-ready).
   For port work, follow the port order in testing-policy.md and the
   trackers on #4.
   A claim, assignment or authored issue under the account you act as
   (`gh api user --jq .login`) is already yours, fresh or stale: resume it
   without asking anyone, and comment that you are continuing it.
2. Prefer, in order: issues that unblock other contributors, bugs with a
   clear failing-test sketch, then the next unit in the port order.
3. Claim it: comment `Claiming: <item>` with a short numbered plan (one PR
   per unit) and when you expect the first PR. The claim bot assigns you
   and reacts 🚀 (it replies only to refuse, and says why). Claim only what
   you are working on now. The claim lapses 24 hours after your last
   progress (a comment, or a commit or review on your PR).
4. **Feature issues** (new behavior, not a port or a bug): check that the
   design is agreed before you code. If the issue lists open questions,
   answer each one in the claim comment under a **Decisions:** heading
   (the default you will build, and why), and split the work so the first
   PR does not depend on an unresolved answer (for example, a pure Rust core
   with Tier 1 tests first; UI, protocol fields and logging in follow-ups).
   A question you cannot pick a safe default for goes on the issue as
   **Decision needed:** and that part waits: the claim bot labels it
   `decision-needed` and blocks your claim for 7 days (renewed by each new
   one) until a maintainer answers `Decided:`. An idea with no agreed design
   belongs in Discussions, not an issue (coordination.md section 5).
   Protocol additions (obs-websocket) are additive fields only.
5. **Security bug?** Apply the two tiers in
   [SECURITY.md](../../../SECURITY.md#two-tiers). Only trusted input can
   reach it (the local user, their own config, plugins, scripts, tests):
   it's a normal public bug, so file, claim and fix it as usual. A remote
   party can reach it (a server, a web page, network media, an
   obs-websocket client, a file someone else sent) or it leaks
   credentials: don't file, claim or push it; report it privately and
   continue on the private fork the maintainers open. Unsure: report it
   privately (a maintainer can move it to public).
6. No issue yet? File one first using the bug or port template
   (coordination.md section 5), then claim it.

## 3. RED, then GREEN

- **Characterization** test first if the unit has none.
- New code (a feature with no C original) still starts RED: Tier 1 tests
  against `todo!()` stubs. Tier 3 applies only to the C behavior it touches
  (show that is unchanged); state which tiers do not apply in the PR.
- **RED commit:** a failing test (or `todo!()` stubs) that shows the bug or
  the missing port. Run it and note how it fails.
- **GREEN commit:** the fix or the port, until the test passes.
- **User-facing features** (UI, protocol output, logging) are not done until
  the real app has run once with the change, through its Tier 4 test or by
  hand. Put the observed output in the PR. Unit tests and a compiling build
  were green when #846 shipped with llvmpipe showing green (#929). If you
  cannot run it, use `Refs #N`, not `Fixes #N`, and say what is missing.
- Ports need every applicable tier: Tier 1 Rust unit, Tier 2 unchanged C
  tests against `libobs` and `libobs-c-reference`, Tier 3 differential, Tier 4 for
  protocol code.
- Never edit an existing C test or public C header to make a port pass.
- Host-specific code goes only in `src/platform/` or `src/ffi/`.
- Rust only through `soldr cargo …` from the repo root; Python only through
  `uv run` / `uvx`.

## 4. Open the PR early

As soon as the RED commit exists, push and open a PR against `master`
(a draft is fine; a red run is expected):

```sh
gh pr create --repo obs-rust/obs-studio --base master --draft
```

For a focused CI/configuration fix, run and record local RED evidence, then
push the separate RED and GREEN commits together when the fix is ready. A
remote RED run is not required. Documentation-only changes need no RED →
GREEN test. This is the exception in coordination.md §4; ports still follow
the normal early-PR workflow.

The description names the RED commit and how it fails, then the GREEN
result. Use `Fixes #N` only when the PR completes the issue, `Refs #N`
otherwise. A user-visible change (behavior, a setting, UI, log output, a
protocol field, a platform, a fix a user can notice) adds or updates its
line in [docs/features.md](../../../docs/features.md) in the same PR
(coordination.md section 4). Check `git diff origin/master --stat` for an accidental
`plugins/obs-websocket` pointer change.

## 5. Local checks (recommended, not required)

PR CI is the merge gate. Local runs give faster feedback:

- `./lint --fix`, then commit what it changed (seconds).
- `./unittest` for Rust Tiers 1-3 and Tier 2 in Docker.
- To let CI skip the Linux test jobs, commit, then stamp HEAD with the
  local gate (the pinned command is in AGENTS.md), and push the stamped
  commit **unchanged**. Any amend, rebase or "Update branch" drops the
  stamp, so rebase before stamping.

If the environment fights you (soldr permissions, disk, Docker), don't
debug it for long: push and let PR CI run. Never `docker system prune`;
check `bosn jobs` before heavy Docker work. If the gate says another gate is
running, wait and retry.

## 6. When CI fails

1. Read the failing log: `gh pr checks <N>`, then
   `gh run view <run> --job <job> --log-failed`.
2. Reproduce locally (`./unittest`, the gate lane, or
   `bosn ci run --workflow <file> --job <job> --trigger pr --wait`).
3. Fix, commit, push once.
4. Re-run a single job only for a clear infrastructure flake
   (`gh run rerun --job <id>`). Never re-run a whole workflow to see if it
   passes, and don't push just to test.
5. macOS and Windows failures: read the `cdb` or `lldb` stack in the Tier 2
   log before you push a guess.

## 7. Land it

- Mark the PR ready when CI is green, then queue it: `gh pr merge <N>`.
  The merge queue reruns CI on the merge result; `CI Gate ✅` decides.
  Rebase by hand only when GitHub reports a conflict. Never push to master.
  An agent may use `gh pr merge --admin` only with explicit maintainer
  authorization for that landing and under coordination.md §8: recheck
  that the queue is empty immediately before merging; otherwise queue and
  jump to the front. Run focused checks, and wait for PR `CI Gate ✅` on
  workflow-graph changes. Record the authorization and evidence in the PR.
  Do not ask again for authorization already given in the session.
- When the issue closes, comment what fixed it, what's left, and the
  follow-up issue for the rest. Tick the tracking-issue checkbox.
- Waiting on something you don't control (a maintainer decision, another
  PR, non-public work): comment `Blocked: <reason>`, with a general reason,
  so the claim lasts 7 days instead of 24 hours (coordination.md section 3).
  Any progress comment ends it.
- Stopping early: comment `Releasing <item>` and link the branch; the
  claim bot unassigns you.
- **Hand off at the merge queue (the default).** Don't wait on the queue.
  Hand the PR to the coordinator and take the next issue once all of these
  are done:
  1. you rebased onto `origin/master` and ran the full local gate
     (`./lint`, `./unittest`);
  2. you stamped HEAD with the local gate and pushed that commit
     unchanged;
  3. you marked the PR ready, turned on auto-merge
     (`gh pr merge <N> --auto`), and commented `Handed off to the
     coordinator`.

  The coordinator then handles rebases, ledger and list conflicts,
  mechanical fixups (lint, formatting, counts), flake reruns, and
  re-queuing after an ejection (obs-coordinator §2). The PR is still
  yours. You keep the claim. A real failure in the PR's own logic comes
  back to you as a coordinator comment, and you fix it before taking
  more work. Don't hand off workflow-graph or release changes, `--admin`
  landings, or user-facing features that still need checking on a
  device: watch those through to merge.

Then return to step 1 for the next issue.

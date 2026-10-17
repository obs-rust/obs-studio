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

## 1. Start from current master

```sh
git fetch origin && git rebase --autostash origin/master
```

Work in your own branch or worktree, named `port/<unit>`, `fix/<issue>-<topic>`,
`test/<unit>` or `infra/<topic>`. Don't touch other people's branches, and
don't delete local branches you didn't create.

## 2. Choose and claim one issue

1. List candidates: `gh issue list --repo obs-rust/obs-studio --state open`.
   Skip anything with a fresh claim (labeled `claimed`, or a `Claiming:`
   comment under 48 hours old, 8 on `critical` issues), and anything with
   an open PR (`gh pr list --repo obs-rust/obs-studio --search "<topic>"`).
   For port work, follow the port order in testing-policy.md and the
   trackers on #4.
2. Prefer, in order: issues that unblock other contributors, bugs with a
   clear failing-test sketch, then the next unit in the port order.
3. Claim it: comment `Claiming: <item>` with a short numbered plan (one PR
   per unit) and when you expect the first PR. The claim bot adds the
   `claimed` label and reacts 🚀 (it replies only to refuse). Claim only what
   you are working on now.
4. **Feature issues** (new behavior, not a port or a bug): check that the
   design is agreed before you code. If the issue lists open questions,
   answer each one in the claim comment under a **Decisions:** heading
   (the default you will build, and why), and split the work so the first
   PR does not depend on an unresolved answer (for example, a pure Rust core
   with Tier 1 tests first; UI, protocol fields and logging in follow-ups).
   A question you cannot pick a safe default for goes on the issue as
   **Decision needed:** and that part waits. An idea with no agreed design
   belongs in Discussions, not an issue (coordination.md section 5).
   Protocol additions (obs-websocket) are additive fields only.
5. No issue yet? File one first using the bug or port template
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
  tests against `libobs` and `libobs-with-rust`, Tier 3 differential, Tier 4 for
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
  Rebase by hand only when GitHub reports a conflict. Workers never use
  `gh pr merge --admin` or push to master: a merge outside the queue
  rebuilds every queued PR. The admin bypass is for maintainers only
  (coordination.md §8).
- When the issue closes, comment what fixed it, what's left, and the
  follow-up issue for the rest. Tick the tracking-issue checkbox.
- Stopping early: comment `Releasing <item>` and link the branch; the
  claim bot removes `claimed`.

Then return to step 1 for the next issue.

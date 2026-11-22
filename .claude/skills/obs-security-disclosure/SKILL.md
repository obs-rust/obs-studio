---
name: obs-security-disclosure
description: What to do with a high-risk security advisory once it has been investigated and triaged. Covers the private upstream-review entry, the fix in the private fork, coordinated disclosure to the OBS Project through the numbered encrypted archives published at obs-rust.com/security, and publishing. Use it whenever you triage, answer, fix or publish an advisory, or when a reporter asks about upstream.
---

# obs-security-disclosure: after the investigation

[SECURITY.md](../../../SECURITY.md) decides the tier and the severity
(obs-coordinator §3a does the first triage). This skill covers everything
after that: how an investigated high-risk advisory reaches the OBS Project,
gets fixed and is published. The public face of the process is the
disclosure tracker at <https://obs-rust.com/security/>.

## The one rule people get wrong

**Upstream hears about an embargoed advisory only through a numbered,
AES-256 encrypted disclosure archive that a maintainer sends.** Nobody
else contacts the OBS Project about it:

- Never tell a reporter to report it upstream themselves, or to use
  obsproject's private vulnerability reporting or security@obsproject.com.
  The answer to "can this go upstream?" is "it goes in the disclosure
  archive", and you name the archive (see step 2).
- Never email, open an issue, a PR or an advisory on obsproject/obs-studio,
  and never comment on an upstream public issue about the cause.
- Never put a GHSA id, title, component, file, CWE or impact class of an
  embargoed advisory on a public page, issue, PR, commit or Discussion. The
  public tracker lists ids only once GitHub shows them as published.

On 2026-11-21 a coordinator brief told an agent to let a reporter take an
embargoed advisory upstream through obsproject's private reporting. That
broke this rule: the advisory had already gone upstream in archive 001.

## Where things live

| What | Where | Who sees it |
|---|---|---|
| The advisory, its comments, its temporary private fork | `obs-rust/obs-studio` Security tab | admins, `security-maintainers`, reporter |
| The upstream review entry for each advisory (`advisories/<GHSA>/README.md`, `meta.json`, `fix.patch`) and the archives with their passwords | private repo `obs-rust/obs-security-upstream-review` | invited members only |
| Upstream's answers (one issue per GHSA id) and the coordinated publication date | issues in `obs-rust/obs-security-upstream-review` | invited members only |
| The encrypted zips, their SHA-256, aggregate counts, published ids | <https://obs-rust.com/security/> (`obs-rust/obs-rust.github.io`: `security/archives.json`, `security/archives/`) | public |

The private repo's `AGENTS.md`, `README.md`, `docs/ARCHIVE_WORKFLOW.md` and
its `obs-security-archive` skill are authoritative for archives. Read them
before you touch that repo. **Never print, quote or log an archive
password.** It is kept only in that repo's `README.md`; refer to it as "in
README.md".

## Steps after triage

1. **Triage is recorded on the advisory** (`## Maintainer triage (<date>)`
   with the tier, severity, where the fix goes and the publishing checks; see
   obs-coordinator §3a). A low-risk report goes to the maintainer to move to a
   public issue. It never gets an upstream entry.

2. **Check whether it has already shipped upstream.** In a checkout of
   `obs-rust/obs-security-upstream-review`, run
   `git grep -l <GHSA> -- 'archives/*/manifest.json'`. A hit means it is in
   that archive. Tell the reporter "it went to the OBS Project in disclosure
   archive NNN". Don't name the archive's contents or its password.

3. **Write the upstream review entry** if it has none:
   `advisories/<GHSA>/README.md` (title, affected upstream files, upstream
   impact, root cause, proposed fix, credits, the advisory text) and
   `meta.json` (copy the fields of an existing entry: `ghsa_id`, `summary`,
   `component`, `upstream_files`, `upstream_impact` one of `upstream-c`,
   `upstream-dependency`, `obs-rust-only`, `obs-rust-infra`,
   `verified_against` (an upstream obsproject/obs-studio master SHA),
   `verified`, `root_cause`, `proposed_fix`, `fix_where`, `patch_state`,
   `has_patch`). Use upstream file names (`obs-video.c`, not
   `obs-video.c.h`), and check the bug against current upstream master by
   reading the code. Commit straight to that repo's `main` and push. The next
   archive then picks it up by its creation date.

4. **Fix it in the advisory's private fork**, RED test then fix (SECURITY.md
   "Fix"). Open the fork with
   `POST repos/obs-rust/obs-studio/security-advisories/<GHSA>/forks` when the
   reporter has a fix ready. Never push the fix to a public branch before
   publication. Once the fix exists, add `fix.patch` to the entry (mapped to
   upstream file names; `patch_state` says whether it applies to upstream
   master) and set `has_patch` and `fix_where`. If the advisory already
   shipped in an archive, the change ships again in the **next** archive as
   an update (`--update <GHSA> --update-reason "fix.patch added"`). Never
   rebuild an archive that has been sent.

5. **Archives are cut, published and sent only on the maintainer's explicit
   instruction**, with the private repo's `obs-security-archive` skill. Steps
   2-4 are what an agent does on its own. Building the zip, publishing it to
   obs-rust.com, emailing the link and sending the password are not.

6. **Answer upstream.** Each tick, read the issues in
   `obs-rust/obs-security-upstream-review` (one per GHSA id). Answer
   questions the advisory and the code settle. A request for a CVE, a
   different publication date or a credit change goes to the maintainer.

7. **Publish** only after SECURITY.md's three "Before publishing" checks
   (the whole thread read, every reproduction failing on master, the
   reporter's confirmation) **and** the coordinated publication date agreed in
   the private repo. Publishing, CVE requests and the date are the
   maintainer's. After publication, add the id to that archive's
   `published_ids` in the site repo's `security/archives.json` (the site build
   accepts only ids that GitHub shows as published), through a PR.

## Never

- Tell anyone to report an embargoed advisory upstream, or contact the OBS
  Project about it yourself.
- Publish an advisory, request a CVE, add a collaborator, or send or publish
  an archive without the maintainer's explicit instruction.
- Reveal an archive password, a manifest, or `advisories/` content outside
  the private repo.
- Name an embargoed advisory (id, title or details) anywhere public,
  including the obs-rust.com site, commit messages and coordinator reports in
  public threads.

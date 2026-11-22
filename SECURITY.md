# Security policy

obs-rust is a fork of OBS Studio that is being converted to Rust. This
policy covers obs-rust: this repository's code, its CI, and the builds
it publishes. The upstream OBS Project has its own policy and channels
(see [Upstream OBS Studio](#upstream-obs-studio)).

## Two tiers

OBS Studio is a desktop application. Its user already runs it with
their own privileges, and it does not run as a public server. So most
bugs are not security issues, even memory bugs. What decides the tier
is who can reach the bug.

| Tier | Who can reach the bug | Where it goes |
|---|---|---|
| **Low risk** | Only trusted input: the local user's own actions, settings, scene collections and profiles, installed plugins and scripts, tests | A normal public issue and PR, like any other bug |
| **High risk** | A remote party: a streaming or WHIP server, a web page in a browser source, media files or streams from the network, an obs-websocket client, a scene collection or profile someone else sent. Also any bug that leaks credentials (stream keys, OAuth tokens, passwords) to another user or over the network | A private security advisory (below) |

When you are unsure, report it privately. A maintainer moves a report
that turns out to be low risk to a public issue, and tells you.

## Low risk: report it in public

Open a bug issue, or a PR with a failing test and then the fix, as
[docs/rust-port/coordination.md](docs/rust-port/coordination.md) says.
Use the title `bug: <component>: <symptom>` with the bug class (for
example use-after-free) in the symptom, so it can be found later. It may
carry the `security` label. No advisory is needed.

## High risk: report it privately

**Do not open a public issue, pull request or discussion for a
high-risk vulnerability.**

1. Open the repository's **Security** tab and choose **Report a
   vulnerability**
   (<https://github.com/obs-rust/obs-studio/security/advisories/new>).
2. Say what is affected (file, function, build or release), who can
   reach it, how to reproduce it, and what an attacker gains. A short
   proof of concept helps.
3. If you already have a fix, say so. A maintainer will open a
   temporary private fork on the advisory, and you can push the fix
   there. Do not push it to a public branch or fork.

**Who can see it.** A private advisory and its private fork are visible
only to the repository administrators, the members of the
`security-maintainers` team (which has GitHub's security manager role
for the organization), the reporter, and the people a maintainer adds
to that advisory. Other contributors, including other agents, do not
see it until it is published. Maintainers add a collaborator to an
advisory only when that person works on its fix.

## What happens next (high risk)

- **Within 72 hours:** a maintainer acknowledges the report on the
  advisory.
- **Triage:** we confirm the issue and agree on its severity (CVSS)
  with you, on the advisory.
- **Fix:** the fix is developed and reviewed in the advisory's private
  fork. It follows the normal rules for this repository: a failing test
  that shows the bug first, then the fix (see
  [docs/rust-port/testing-policy.md](docs/rust-port/testing-policy.md)).
- **Before publishing:** a maintainer checks each of these and records
  it on the advisory:
  1. **Read the whole comment thread**, not only the description. The
     reporter may have added findings since the report. Comments close
     when the advisory is published, so this is the last chance to
     answer them privately.
  2. **Every path in the report is fixed**, not only the first one. Run
     each reproduction in the report (every variant: each function,
     each code path) against current master, and name the PR that fixes
     each one. A fix that landed outside the advisory, in a public PR,
     counts only after this check.
  3. **The reporter confirms the fix** on the advisory. If they have not
     answered within 72 hours of being asked, publish anyway and say
     so.
  If any check fails, the advisory stays private and the remaining
  part is fixed first.
- **Release and disclosure:** we merge the fix, publish the advisory
  (requesting a CVE where it applies), and credit you unless you ask us
  not to. We aim to disclose within 90 days of the report, sooner once
  a fixed build is out.

After disclosure, any public follow-up work (hardening, regression
tests, release notes) is tracked in normal issues with the `security`
label. The label is never used for an undisclosed high-risk
vulnerability.

## Upstream OBS Studio

Much of obs-rust's C/C++ code comes from OBS Studio. If a vulnerability
also affects upstream OBS Studio:

- Report it to obs-rust as described above. **Do not report it upstream
  yourself**, and do not discuss it on an upstream issue.
- obs-rust discloses to the OBS Project in a coordinated way. Embargoed
  advisories go to the OBS Project in numbered, AES-256 encrypted
  disclosure archives. The maintainers send each archive's link, and its
  password over a separate channel. The OBS Project reviews and fixes
  first, and both projects then publish on an agreed date. The public
  tracker at <https://obs-rust.com/security/> shows each archive, its
  SHA-256 and aggregate counts, but never the title or id of an
  embargoed advisory.
- An advisory goes into the first archive cut after it is created. A fix
  found after its archive was sent goes out as an update in the next
  archive. You are credited in the archive unless you ask us not to.
- If you have already reported it upstream, tell us, so that the two
  reports share one disclosure date.

Agents follow the `obs-security-disclosure` skill
([.claude/skills/obs-security-disclosure/SKILL.md](.claude/skills/obs-security-disclosure/SKILL.md))
for every step after triage.

A bug that affects only upstream OBS Studio, and not obs-rust, should
go to the OBS Project directly: <https://github.com/obsproject/obs-studio/security/policy>.

## Scope

In scope:

- Code in this repository, including the Rust ports, the C/C++ code
  inherited from upstream, plugins and scripting.
- Network-facing interfaces, such as the obs-websocket server, RTMP,
  SRT, WHIP and other outputs, and media inputs that parse files or
  streams.
- Builds and release artifacts published by this repository (AppImage,
  Flatpak, DMG, Windows packages) and the CI that produces them.

Out of scope:

- Attacks that require physical access to the device, or that require
  the victim to install non-standard software or take active steps to
  make themselves vulnerable (this includes DLL planting).
- Reports from automated tools or scans that have not been checked by
  hand.
- Vulnerabilities in third-party dependencies that obs-rust does not
  change. Report those to the dependency's maintainers; tell us as
  well if obs-rust ships an affected version.

## Safe harbor

We will not pursue or support legal action against research done in
good faith under this policy. Good faith means avoiding privacy
violations, data destruction and service disruption, testing only
against your own installations, and keeping details confidential until
the advisory is published.

## No bug bounty

obs-rust does not offer payment for reports.

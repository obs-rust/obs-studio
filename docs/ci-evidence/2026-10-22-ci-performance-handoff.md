# CI performance handoff — 2026-10-22

## Status and ownership

The user paused the broad #1667 goal and requested this handoff. **The goal is
not complete.** Primary current issue: **#1801**, cache allocation/retention.
Its AppImage child **#1832** also remains open. No next unrelated issue is claimed.
This draft PR contains the current native measurement-routing work and all
remaining owned scratch captures. Continue from this branch in this checkout;
do not create subtrees/worktrees or duplicate current claims.

Read `AGENTS.md`, `.claude/skills/obs-worker/SKILL.md`, coordination/testing policy
and `docs/ci-tooling.md`. Default branch is **master**, not main. Fetch/rebase
onto `origin/master` before starting the next unit. Preserve the normal queue
and PR `CI Gate ✅`; no bypass or blind whole-workflow retries.

## Current unmerged work

Branch: `infra/1801-native-measurement-scope`.

- RED **`fe42cd3af`**: 11 routing tests, 19 expected failures before implementation.
- GREEN implementation in this draft adds default-off
  `native-cache-measurement-only` to existing workflows. It accepts only trusted
  same-repository master manual runs with exactly one existing native transport
  or reuse protocol. Release/full overrides and conflicting Flatpak/setup/budget
  requests fail validation before heavy work.
- Keep all four native app axes, their current compiler/profile/integrity and
  source-overlay freshness checks, requested publisher/consumer protocols, and
  lightweight gates. Omit unrelated matrices only for this declared experiment.
  Routine PR/queue/push/schedule/release/ordinary-dispatch selection stays intact.
- No workflow, job, native compiler invocation or test executable was added.
  No hosted experiment was dispatched for this change.
- **24 focused tests passed** (`ci.test_flatpak_measurement_scope`,
  `ci.test_native_cache_transport_workflow`, `ci.test_tier2_setup_probe`).
  Full `./lint` **passed in 51.5 s**. Sole primary clud-review found no material
  issues; its focused routing/setup subset passed 20 tests.
- First lint attempt caught an overbroad setup-probe test forbidding any mention
  of its flag. It now checks the actual normal-job selector/direct-input invariant;
  added tests evaluate the real conflicting-input expressions and caller routing.
  The earlier failed lint log was overwritten by the successful rerun; do not
  claim that raw failed log is in the bundle.

This is a **draft handoff**, not a measured speedup, native backend adoption,
complete cache allocation or full-platform qualification. Review PR CI before
queuing it. No automatic experiment should follow merely because it merges.

## Problematic areas and why they remain open

### 1. Global capacity is still overcommitted — #1801

After #1826, declarations are **13,332,643,839 B** against **10,737,418,240 B**:
**2,595,225,599 B over budget**. Snapshot headroom does not establish durable
retention, especially while generations overlap or architecture entries are absent.
The [25-family report](2026-10-22-cache-allocation-followup.md) maps writers,
profiles, observations and remaining investigations. Not every family has complete
transfer/net-benefit/growth evidence.

Prepared native architecture PR **#1723 remains draft, unqueued, disposition
revise**. After the Flatpak retirement it would still declare16,224,197,016B,
5,486,778,776B above budget. Independent newest-entry protection cannot create
storage. Do not merge it unchanged, raise paid capacity or shrink declarations
and call the allocation solved. The recorded350M Windows trial nearly doubled
rebuild time and remains rejected.

### 2. Distinct Actions versions are treated as superseded

Existing owner: [zackees/ci.yml#75](https://github.com/zackees/ci.yml/issues/75).
The [merged allocation report](2026-10-22-cache-allocation-followup.md) includes
actual container registry consumption and later logged janitor deletion in favor
of a different-version host archive under the same key. #1354 namespaced cook
archives but left registry/mini unchanged.

Additional retained observations:

- Master janitor120101520370 in39858120197 deleted host mini9363144969 at21:32:55Z,
  keeping container9366966283. This is an actual deletion. It is the reverse of
  the earlier saved hypothetical plan after access order changed.
- Queue janitor120104960166 in39859128832 later logged another registry deletion.
- Host PR Fast Gate120106672085 and later master Fast Gate120105801935 both
  restored a mini archive successfully. Inventory lists replacement host
  ID9367684630. Do not turn these HITs into asserted misses, or attribute the
  replacement producer/intervening deletions solely from presence/absence.
- Current findings are posted on [upstream #75](https://github.com/zackees/ci.yml/issues/75#issuecomment-6352077570).
  Original older Qt/Flatpak disappearance actors remain unknown.

Next: saved-version RED→GREEN at the existing owner, then qualified pin/compatible
archive strategy and real later consumers. Do not patch the installed janitor
ad hoc or remove useful versions to satisfy arithmetic.

### 3. AppImage producer repair is merged; qualification is not — #1832

**#1833 merged at `212f8c0240de32c3afdec20c739ea5dc678ddd01`.** The launcher now
forwards only `ZCCACHE_CACHE_DIR` into the existing workspace bind mount.
[Evidence](2026-10-22-appimage-cache-forwarding.md) preserves the old actual failed
writer:314cold/0cached calls, then missing `.zccache` at save. Local RED/GREEN,
10 focused tests, lint83.3s and review passed. Required PR/queue gates passed.

Still needed: trusted nonempty publication, later compatible compiler hits,
source-edit correctness, restore/save/build costs and durability within budget.
No post-fix native reuse/performance claim has been made.

Existing [zccache#1935](https://github.com/zackees/zccache/issues/1935#issuecomment-6244841263)
records stale native GCC output after an immediate natural same-size edit on the
pinned1.15.5. That within-session case is not proof of the identical cross-run
AppImage failure, but hits alone cannot satisfy source freshness. Do not insert
sleeps, weaken controls or change pins without qualification to obtain green.

Use normal traffic where possible. Existing AppImage build39845783549 took702s
job wall (image260s, configure14s, build335s); its Ubuntu runtime lane recorded
1908step-seconds. These are single observations, not billed runner time. No costly
full matrix was added to PR CI or dispatched for the cheap forwarding fixture.

### 4. Native release transport remains default-off — #782 / #1801

The existing [native build-reuse report](2026-10-22-native-cache-build-reuse.md)
retains the failed qualification: Windows strict compiler/SDK identity misses;
macOS real compiler reuse but failed installed-binary freshness harness. #1792
fixed the harness later; that does not retroactively pass the original trial.

Do not relax profile matching or call archive restoration alone qualification.
The routing change in this draft reduces the cost of a future *planned* experiment
using existing native jobs. Before any dispatch, specify missing evidence,
producer source/profile, cost/storage, and ensure no existing manual master run
would be cancelled by the shared concurrency group. Preserve all results.

### 5. Subsequent master observation failed before cache restore

Run **39858120197**, source233db443a, is terminal **FAILURE**:
10 successful jobs,16 skipped,1 failed. Linux Tier2 build120105801763 failed
`Install Git` at21:42:23–21:49:40UTC (437s), exit100, after Ubuntu archive/security
HTTP:80 connection timeouts. Checkout, cache restore, compilation and tests did
not execute in that job. It is not a later cache-miss or prepared-inventory result.

Raw completed-job log, job metadata and terminal run metadata are in the companion.
Existing owner #1592 has [the observation](https://github.com/obs-rust/obs-studio/issues/1592#issuecomment-6352101633).
No retry, replacement dispatch, cancellation or weakened test was used. Later
existing master runs remain distinct handles; revalidate their state before use.

## Completed units and broader scope

- #1826 merged233db443a: retire obsolete Flatpak migration restores and200MiB
  reservation, with RED/GREEN, lint, review and both gates green.
- #1830 merged209d977f3: reproducible25-family allocation/version report.
- #1833 merged212f8c024: AppImage environment repair, qualification still open.
- #1269 original incident/report, #1581 and #1694 bounded report batches are
  closed against their own scope. Their completion does not finish #1667.
- #930 and #1286 remain broad active tracks. Do not mark the under-eight-minute
  code queue goal achieved from docs-only or unmatched cohorts.

## Evidence transfer and cleanup

The companion `2026-10-22-ci-performance-handoff.json.gz` preserves **47 exact
remaining files** from the two owned task scratch roots, with per-record length,
SHA256 and base64 bytes. It includes raw additional janitor/consumer/bootstrap
logs, terminal failed-master metadata, the first projected Flatpak consumer,
local current routing RED/GREEN/lint output, historical comments/configurations
and AppImage handoff notes. Historical or active-at-capture metadata is not a
current-state claim. Previously merged bundles remain canonical for their units.

Companion SHA256:
`9646c31479e750ea48056ec6a47e6e5e168872a3fbdb1c4197574bfe0fbea6c5`.

Verify without executing builds:

```sh
uv run --no-project - <<'PY'
import base64, gzip, hashlib, json
from pathlib import Path
bundle = Path('docs/ci-evidence/2026-10-22-ci-performance-handoff.json.gz')
records = json.loads(gzip.decompress(bundle.read_bytes()))['records']
assert len(records) == 47
for record in records:
    raw = base64.b64decode(record['base64'], validate=True)
    assert len(raw) == record['bytes']
    assert hashlib.sha256(raw).hexdigest() == record['sha256']
print('47 handoff records verified')
PY
```

Before handoff,26 redundant captures were verified against merged#1830 and
removed with safe-rm; five more were verified against merged#1833 and removed.
No worktree/subtree was created. The remaining scratch roots can be removed with
safe-rm after this handoff bundle is verified in the pushed PR. Do not delete
unrelated agent directories, branches, shared caches or build state.

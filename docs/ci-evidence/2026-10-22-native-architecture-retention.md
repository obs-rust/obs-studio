# Native architecture retention candidate (#1269)

## Observed defect

Janitor job119654117873 in trusted queue run39727319824 completed SUCCESS
and directly reported deletion of current master Windows x64 seed9320261567
(1,063,364,663 bytes) because its pooled Windows family exceeded800MiB.
ARM64 seed9319978189 (901,034,946 bytes) remained. This is an observed
janitor deletion, distinct from earlier hypothetical replays. The deleted
older macOS x64 entry had a newer same-architecture replacement; do not call
that loss of an architecture seed.

[Evidence bundle](2026-10-22-native-architecture-retention.json.gz) retains
queue source/job metadata, exact configuration, raw-log hash and literal
excerpts, plus replay inventories/configurations/reports. The queue config
matches the configuration captured alongside the observed post-sweep listing.
Usage and listing observations remain non-atomic; no attribution is made for
missing entries not explicitly identified by this sweep.

## Candidate, not adopted

Separate first-match families for Windows x64/ARM64 and macOS x86_64/ARM64
prevent one architecture from substituting for another's newest seed.
Observed archive sizes support these honest allowances:

| Seed | Observed archive bytes | Candidate allowance |
| --- | ---: | ---: |
| Windows x64 | 1,063,380,057 | 1.3 GiB |
| Windows ARM64 | 901,055,569 | 1.1 GiB |
| macOS x86_64 | 675,951,544 | 800 MiB |
| macOS ARM64 | 659,815,401 | 800 MiB |

No working-store limit, writer trust, cache key/path, workflow or build/test
coverage changes. The test key inventory resolves actual workflow matrix
architectures, including release include rows, so specific prefixes remain
checked rather than weakening coverage to accept generic unresolved keys.

Combined native declarations rise from1,363,148,800 to4,254,701,977 bytes.
Total declarations rise from13,542,359,039 to**16,433,912,216 bytes**, now
**5,696,493,976 above10GiB**. These allowances do not create physical
capacity or guarantee GitHub retention during writer bursts. **Hold this
candidate draft until global allocation/transport is reconciled.** No paid
capacity, hidden backend or global-success claim. The pruned Flatpak rebuild
experiment remains a separate live unit under #1269.

## RED → GREEN and production replay

RED `ce50253e1`: two focused cases fail on shared undersized families and
unresolved workflow architecture keys. GREEN focused checks pass15 tests.

Pinned production janitor `1a970f01574447d681571ff3d0b8de56cb983dcc`, same
saved real inventory, +601-second after-grace clock, no delete callback:

- Before: selects current Windows x64 ID9318776303 and macOS ARM64
  ID9318871800 for eviction.
- After: selects none of the four current native seeds for eviction.
- Additional explicitly synthetic fixture: adds four older same-architecture
  entries (IDs9900000000–9900000003). After selects all four older entries,
  while preserving the real current seeds. These injected entries and their
  resulting inventory totals are fixtures, not hosted observations.

Reproduce any bundled replay with the existing pinned audit helper:

```sh
native_replay_root=$(safe-mktemp /tmp/obs-native-retention-replay)
uv run --no-project - "$native_replay_root" after <<'PY'
import gzip
import json
import sys
from pathlib import Path
record = json.loads(gzip.decompress(Path('docs/ci-evidence/2026-10-22-native-architecture-retention.json.gz').read_bytes()))
replay = record['replays'][sys.argv[2]]
root = Path(sys.argv[1])
(root / 'inventory.json').write_text(json.dumps(replay['inventory']))
(root / 'ci.toml').write_text(replay['ci_toml'])
PY
uv run --no-project docs/ci-evidence/2026-10-21-cache-retention-audit.py --output "$native_replay_root" --replay
safe-rm -r "$native_replay_root"
```

Use before, after or superseded-fixture as the second extraction argument.
No live sweep, stable day, end-to-end speedup or completed #1269 is claimed
for this unadopted candidate.

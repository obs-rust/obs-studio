# Shared registration conflicts

Oracle ports register complete-source extracts in
`rust/obs-c-oracle/oracle.d/<output-stem>.list`, one extraction per file:

```text
hotkey_register.inc|libobs/obs-hotkey-register.c.h|auto
```

The fields are the output filename, repository-relative source path and PCH
marker. `auto` uses the existing libobs/graphics PCH selection; an explicit
`#include "libobs/build/example.pch.h"` strips that exact marker instead.
Extraction still uses the original source and the same `#line` directive.
Do not append complete-source registrations to `build.rs`. Wrapper `.c` files
continue to be discovered from `oracle/`; field adapters keep their discovery
rule. Special multi-function extracts and transformations remain in the script.

The build watches the fragment directory, including additions and removals,
and each source. Its native input hash includes fragment names and contents,
the reader and every selected source. Duplicate outputs, malformed fragments
and paths outside the repository fail the build rather than omit an oracle.
The Utilities test category checks entry discovery, editing, removal and invalid
registrations; the Python lint checks reject new shared extraction blocks.

Two native registration snapshots are mechanical slices of the canonical graph:
`headless_characterizations_expected.json` and `portal_output_expected.json` in
`rust/obs-build/tests/native/registration/`. After editing their graph entries,
regenerate them with:

```sh
uv run ci/native_expectations.py --fix
```

Running without `--fix` checks for drift; the same check runs in `./lint`'s
Python self-tests. The selected cases, array order, complete target/test fields
and independent Rust host-selection/count assertions are preserved. A missing
canonical case is an error. Do not hand-merge these snapshots. `ci/ai_rebase.py`
resolves the canonical graph first, checks that source resolution, regenerates
conflicted snapshots and stages them. Generated-only conflicts need no model;
invalid graph JSON or missing cases refuse the rebase. Other expected files
remain independent fixtures and are outside this regeneration allowlist.

The canonical graph's independent-fragment migration is tracked separately in
PR #4655. Until it lands, a graph conflict still needs source resolution before
snapshot regeneration. README rows and unity groups also remain shared files.

Measurements for Refs #4896 were captured on 2026-11-17 UTC at master
`21d62feeb8010da3a1b1de85206e71bd4018953e`, using
`git log origin/master --since=2026-11-01 --format= --name-only` and
`git merge-tree --write-tree --name-only` on 36 pairs of current heads of PRs
#4915, #4920, #4921, #4922, #4925, #4931, #4933, #4935 and #4939.
There were 9 conflicting pairs: all 9 hit `obs-c-oracle/build.rs`, and 8 also
hit `libobs/build/libobs.graphics+.h.c`. #4920/#4922 hit only the oracle script.
November churn was 244 commits for `rust/README.md`, 210 for `test-graph.json`,
143 for the core-source unity group, and 50 for the oracle script. Current heads
already include mechanical rebases, so this is a lower bound on past conflicts.
Replaying #4931's pre-rebase head `e5399c639334b01325afd0e6a114ae88ac13d8ce`
against master `aace99f022e7` reproduces the reported graph and both snapshot
conflicts. The fix removes the whole-source oracle append point and replaces
manual recovery for those two snapshots with deterministic regeneration. It does
not claim to eliminate the remaining graph, unity-group or README conflicts.

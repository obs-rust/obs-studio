# Linux Tier 2 validation

`validate.sh` runs the unchanged cmocka suites against both the original
C libobs (`OFF`) and libobs with Rust implementations (`ON`). The default
local stage builds and tests both modes. CI splits the stages to reuse one
build artifact without compiling again in each test job.

## Build and test contract

- `OBS_TIER2_STAGE=build`, `OBS_TIER2_MODES="OFF ON"`: build both
  implementations, compare exports, and pack the relocatable test tree in
  `OBS_TIER2_PACK`. Compiler objects are excluded from the artifact.
- `OBS_TIER2_STAGE=test`: unpack that tree at `OBS_TIER2_BUILD`, install
  the runtime packages, and run CTest. This stage needs no Rust toolchain.
- CI runs two complete test jobs, one per mode, with
  `OBS_TIER2_MODES` set to `OFF` or `ON` and `OBS_CTEST_JOBS=8`.
  There is no shard filter. Failure in one mode does not cancel the other.
- The aggregate Tier 2 job and lane proof require the entire matrix to
  succeed. A single passing mode cannot establish implementation parity.

## Test timing persistence

`OBS_TIER2_COST_DIR` supplies historical CTest costs for longest-first
ordering and receives refreshed `ctest-cost-OFF.txt` or
`ctest-cost-ON.txt` after testing. Each matrix runner uploads only its own
mode under a distinct `tier2-cost-<mode>` artifact. Hidden-file upload must
be enabled because the source directory is `.tier2-cost`.

Master combines the artifacts with `merge_costs.py`, validates that both
modes contain measured tests with `check-costs.py`, then saves the cache
for subsequent runs. Never save an empty cache as successful measurement.
The downloader may flatten a single matching artifact, while multiple
artifacts have named subdirectories; consumers must support both layouts.

For a scheduling change, verify real CI logs show both full suites and
compare their test counts to the prior revision. Report elapsed gate time
and summed runner time separately: concurrent modes reduce wall time but
repeat runner setup. Local shell-policy tests alone do not establish that
the matrix ran or that queue throughput improved.

## Running the RUN_SERIAL part beside the suite

CI gives the `RUN_SERIAL` tests their own runner (`OBS_CTEST_PART=serial`).
A local host has no second runner, so `OBS_TIER2_SERIAL_CPUS=<cpu list>`
gives them their own cores instead: the suite part runs `ctest -E` pinned to
the CPUs the variable leaves, while `serial-runner.py` runs each `RUN_SERIAL`
test as its own process on its own CPU pair, at the same time.
`tree.ctest-<mode>.log` and `tree.serial-<mode>.log` are both kept, and
either part failing fails the mode. Unset, the lane is unchanged; CI never
sets it, and it cannot be combined with `OBS_CTEST_PART` or
`OBS_CTEST_SHARD`. The `run.py` harness passes it into the container, so
`OBS_TIER2_SERIAL_CPUS=0-9 uv run --script rust/tools/linux-validate/run.py`
is the whole invocation on a 16-core host.

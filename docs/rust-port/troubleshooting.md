# Troubleshooting your environment

Local runs are optional: PR CI is the merge gate
([coordination.md](coordination.md)). **If your environment fights you, open
the PR (a draft is fine) and let CI run.** A red local run caused by your
machine says nothing about your change.

`./lint` and `./unittest` print a one-line `hint:` that points here when they
see one of these errors.

## `soldr: IO error: Permission denied (os error 13)`

**Cause:** soldr cannot write its cache (`~/.soldr`) or the Cargo `target/`
directory. Usually one of them was created by another user, by `sudo`, or by a
container running as root, or `target/` is shared between users or runners.

**Fix:** give the directories back to your user:

```sh
sudo chown -R "$(id -u):$(id -g)" ~/.soldr target
```

Don't run the gate with `sudo`. On a self-hosted runner, give each runner
user its own home directory and checkout instead of a shared `target/`.

## `soldr: command not found`

**Cause:** soldr is not installed, or not on `PATH` in this shell. The gate
runs every Rust tool through soldr (AGENTS.md), so bare `cargo` is not a
substitute.

**Fix:** install it with `uv tool install --upgrade soldr`
([zackees/soldr](https://github.com/zackees/soldr)), then open a new shell or
run `uv tool update-shell` so uv's tool directory is on `PATH`
(`command -v soldr` should print a path).

## `soldr: warning: disk space is low (… free)`

**Cause:** less than 10 GiB is free on the volume that holds the build. Builds
then fail in confusing ways (truncated objects, linker errors, Docker layer
errors).

**Fix:** run `soldr gc` to review and remove stale Rust target directories,
and remove only your own old Tier 2 build volumes: list them with
`docker volume ls --filter name=obs-rust-linux-validate-` and
`docker volume rm` the ones your clone created. Never run
`docker system prune` or `docker volume prune`: on a shared machine they
destroy everyone's warm caches. Check `bosn jobs` before heavy Docker work,
and leave bosn-owned volumes to bosn
([ci-tooling.md](../ci-tooling.md#bosn-docker-under-supervision)). Keep at
least 10 GiB free.

## Tier 2 fails against host packages (FFmpeg 7.1 and libobs needs 8.0)

`Could NOT find FFmpeg … (Required is at least version "8.0")`, or other
missing or too-old libraries.

**Cause:** Tier 2 was built natively against the host's packages. libobs
needs newer versions (FFmpeg 8.0) than many distributions ship.

**Fix:** Tier 2 runs in Docker. Use `./unittest` (or
`rust/tools/linux-validate/run.py`), which builds in an image with the right
dependencies. Don't run Tier 2 against host packages.

## `Cannot connect to the Docker daemon`

**Cause:** Docker is not installed, not running, or your user cannot use it.

**Fix:** start Docker, and add your user to the `docker` group (log out and
back in) so `docker ps` works without `sudo`. If you can't run Docker, skip
`./unittest` and let PR CI run Tier 2.

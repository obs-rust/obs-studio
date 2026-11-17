# Dell Tier 2 package image

The maintainer approved revisiting #966 on 2026-11-17. This image changes only
when packages are installed: it retains the exact stock Ubuntu base digest,
root user and sudo model. It installs the existing Install Git package list and
`../apt-packages.txt` without recommendations. No Rust toolchain or additional
tools are baked in.

`ci/tier2_image.py` is the single production identity helper. Its SHA-256 tag
covers the exact Dockerfile, base digest and both package lists, including
comments. Both publication and selection call it. The independent `tier2-image`
job in `push.yaml` publishes only on this repository's master pushes and only
when the tag is absent. Only that job has `packages: write`. Probe failures stop
the publisher, rather than overwriting an unknown existing tag. The builder's
authenticated probe supports the first publication of a new package.

**The GHCR package `obs-rust/tier2-linux` must be public.** On first publication,
a maintainer must check its package settings and change visibility to public if
GHCR created it private. Linkage to this source repository is declared by the
OCI source label. Consumers use anonymous manifest probes and pulls; no package
credentials or extra permissions are granted to PRs, forks or merge groups.
Until a public manifest is available, they retain their existing image paths.

The first master push after merge builds the image. Dell switches automatically
once the tag's public manifest exists. `check-event` probes only with nonempty
`mq-heavy-labels`, resolves the manifest digest and passes it to the build and
OFF/ON test jobs. Missing tags, private packages and any probe error retain the
stock build image and the tests' existing verified prepared-runtime/stock
fallback. Hosted runners and forks keep those paths and never probe GHCR.

The build keeps its bootstrap and flock-protected apt install steps. The
prebaked bootstrap skips the index update, so it cannot upgrade the baked
versions. The Tier 2 apt install and repair are no-ops. Archive directories
still exist, but Actions archive restores/saves are skipped. OFF/ON tests now
use `ci-runtime.py`, rather than job containers with full-package installs:
they pull the same selected digest, run the existing exact-version closure
install without an index update, and still reject a mismatched dpkg inventory
before running the complete requested suite. All stock runtime behavior stays
in place. Preserve #982 coverage by validating the real suites after rollout.

Package versions are frozen at image build time. Stock installs see the
archive's current versions, so Dell and hosted versions can diverge. An input
change creates a new tag; for an intentional package refresh with unchanged
lists, change a dated comment in the Dockerfile. Never overwrite a published
content tag. The package closure recorded by the build still binds each test
job to its build's versions.

Local verification uses the existing bounded Docker task harness:

```sh
uv run --script ci/docker_task.py run --name tier2-prebake \
  --image obs-rust-tier2-prebake \
  --dockerfile rust/tools/linux-validate/tier2-image/Dockerfile \
  --context rust/tools/linux-validate --source ro -- \
  bash -ec 'grep -v "^#" /src/rust/tools/linux-validate/apt-packages.txt | xargs sudo eatmydata apt-get install -y --no-install-recommends'
```

This local harness uses its own scoped local tag; the production GHCR tag is
printed by `uv run --no-project ci/tier2_image.py identity`.

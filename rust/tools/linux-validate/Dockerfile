# Linux build environment for Tier 2 validation (docs/rust-port/testing-policy.md).
FROM ubuntu:26.04
# The source-manifest validator runs Python through uv in local containers too.
COPY --from=ghcr.io/astral-sh/uv:0.13.5 /uv /uvx /usr/local/bin/
ENV DEBIAN_FRONTEND=noninteractive
COPY apt-packages.txt /tmp/apt-packages.txt
RUN apt-get update && grep -v '^#' /tmp/apt-packages.txt | xargs apt-get install -y --no-install-recommends \
 && if apt-cache show libvpl-dev >/dev/null 2>&1; then apt-get install -y --no-install-recommends libvpl-dev; fi
# libvpl-dev is amd64-only. validate.sh leaves QSV off, so an arm64 image skips it.
RUN uv run --no-project --offline --python /usr/bin/python3 python -c 'import json, pathlib, re'
# Install exactly the toolchain pinned in rust-toolchain.toml (run.sh passes it)
# so corrosion/CMake finds it; rustup must not have to fetch it at run time.
# Compiler launcher for the CMake builds: zccache, pinned with its checksum.
# The installer is a Python script (install_zccache.py); this image already has
# uv from the ghcr layer above, so it runs through `uv run` like the repo's
# other ported helpers, against the container's own /usr/bin/python3.
COPY install_zccache.py /tmp/install_zccache.py
RUN uv run --no-project --offline --python /usr/bin/python3 /tmp/install_zccache.py /usr/local/bin && rm /tmp/install_zccache.py
ARG RUST_TOOLCHAIN
RUN test -n "$RUST_TOOLCHAIN" \
 && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "$RUST_TOOLCHAIN" -c rustfmt -c clippy
ENV PATH=/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

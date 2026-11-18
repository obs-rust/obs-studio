FROM ubuntu:26.04@sha256:f144425ff09be612d6d9ad965196e9cdc23dae1f42110a8a11a3e9a8198759f7
LABEL org.opencontainers.image.source="https://github.com/obs-rust/obs-studio"
COPY apt-packages.txt tier2-image/install-git-packages.txt /tmp/tier2-packages/
RUN sed -i -e 's/^Suites: resolute resolute-updates resolute-backports$/Suites: resolute resolute-updates/' \
      -e 's/^Components: main universe restricted multiverse$/Components: main universe/' \
      /etc/apt/sources.list.d/ubuntu.sources \
    && apt-get update -qq \
    && packages="$(grep -hv '^#' /tmp/tier2-packages/*.txt)" \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends $packages \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* /tmp/tier2-packages

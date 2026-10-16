# Linux build environment for Tier 2 validation (docs/rust-port/testing-policy.md).
FROM ubuntu:26.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
  build-essential cmake ninja-build pkg-config git ca-certificates curl file \
  libglib2.0-dev libcurl4-openssl-dev \
  libffmpeg-nvenc-dev \
  libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev libavutil-dev libswresample-dev libswscale-dev \
  libjansson-dev libx11-xcb-dev libgles2-mesa-dev libwayland-dev libpipewire-0.3-dev libpulse-dev libx264-dev \
  libmbedtls-dev libgl1-mesa-dev uthash-dev libsimde-dev libluajit-5.1-dev python3-dev \
  libx11-dev libxcb-randr0-dev libxcb-shm0-dev libxcb-xinerama0-dev libxcb-composite0-dev libxinerama-dev libxcb1-dev libxcb-xfixes0-dev \
  swig libcmocka-dev libxss-dev libglvnd-dev libxkbcommon-dev libxcomposite-dev libxdamage-dev \
  libasound2-dev libfdk-aac-dev libfontconfig-dev libfreetype-dev libjack-jackd2-dev libsndio-dev libspeexdsp-dev libudev-dev \
  libv4l-dev libva-dev libvlc-dev libpci-dev libdrm-dev nlohmann-json3-dev libwebsocketpp-dev libasio-dev libqrcodegencpp-dev \
  librist-dev libsrt-openssl-dev \
  extra-cmake-modules rsync ccache xvfb xauth pipewire dbus-daemon vlc-plugin-base libxcb-xinput-dev libgl-dev \
 && if apt-cache show libvpl-dev >/dev/null 2>&1; then apt-get install -y --no-install-recommends libvpl-dev; fi
# libvpl-dev is amd64-only. validate.sh leaves QSV off, so an arm64 image skips it.
# Install exactly the toolchain pinned in rust-toolchain.toml (run.sh passes it)
# so corrosion/CMake finds it; rustup must not have to fetch it at run time.
ARG RUST_TOOLCHAIN
RUN test -n "$RUST_TOOLCHAIN" \
 && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "$RUST_TOOLCHAIN" -c rustfmt -c clippy
ENV PATH=/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

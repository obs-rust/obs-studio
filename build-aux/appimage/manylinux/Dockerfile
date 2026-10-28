# glibc 2.28 build environment for the AppImage (obs-rust/obs-studio#926).
# manylinux_2_28 is AlmaLinux 8 (glibc 2.28) with gcc-toolset-14 as the
# default compiler. Libraries AlmaLinux 8 ships new enough come from dnf; the
# rest (Qt 6, FFmpeg and its codecs, PipeWire, GLib, fdk-aac, qrcodegen and
# header-only libraries) are compiled once per version by obs-rust/obs-studio-libs
# (recipes/linux) and fetched here, so this image compiles nothing but OBS.
# The -devel packages also cover what the fetched libraries' .pc files require
# (GIO: libmount; GObject: libffi), so pkg-config can resolve them.
FROM quay.io/pypa/manylinux_2_28_x86_64@sha256:4131ddd7acb9c6c966af02fe7e71edc51e078c2d41f475966697676430bfa2ae

# A stalled mirror served packages at under 1 kB/s until the image step timed
# out (run 40145813015): drop a mirror that stays below 50 kB/s for 30 s,
# prefer the fastest ones and retry each package (#2182).
RUN printf '%s\n' retries=10 timeout=30 minrate=50000 fastestmirror=True max_parallel_downloads=10 >> /etc/dnf/dnf.conf

RUN dnf install -y --setopt=install_weak_deps=False \
        which patchelf file desktop-file-utils squashfs-tools zstd \
        libX11-devel libXext-devel libXfixes-devel libXrender-devel libXcomposite-devel \
        libXinerama-devel libXrandr-devel libXi-devel libXcursor-devel libxshmfence-devel libXScrnSaver-devel \
        libxcb-devel xcb-util-devel xcb-util-wm-devel xcb-util-image-devel xcb-util-keysyms-devel \
        xcb-util-renderutil-devel xcb-util-cursor-devel libxkbcommon-devel libxkbcommon-x11-devel \
        wayland-devel wayland-protocols-devel extra-cmake-modules \
        mesa-libEGL-devel mesa-libgbm-devel libglvnd-devel libdrm-devel libva-devel \
        vulkan-headers vulkan-loader-devel \
        fontconfig-devel freetype-devel glib2-devel dbus-devel systemd-devel \
        alsa-lib-devel pulseaudio-libs-devel speexdsp-devel libv4l-devel \
        pciutils-devel libuuid-devel jansson-devel libcurl-devel openssl-devel \
        luajit-devel uthash-devel websocketpp-devel \
        libmount-devel libffi-devel \
 && dnf clean all

# uv, from obs-rust/obs-studio-libs (the first tool, since it runs the fetch of
# the others), pinned by version and checksum.
ARG UV_VERSION=0.13.5
ARG UV_SHA256=8033f23836f7ffe5f9cfb1620beb830d34c7c45226b96512bee9ffd3fdb3dce1
RUN curl -fsSL --retry 5 --retry-connrefused --retry-delay 5 -o /tmp/uv.tar.zst "https://github.com/obs-rust/obs-studio-libs/releases/download/uv-${UV_VERSION}/uv-${UV_VERSION}-linux-x86_64.tar.zst" \
 && echo "${UV_SHA256}  /tmp/uv.tar.zst" | sha256sum -c - \
 && zstd -dc /tmp/uv.tar.zst | tar -xf - -C /usr/local/bin \
 && rm /tmp/uv.tar.zst

# --- prebuilt dependencies -----------------------------------------------------
# From the root manifest.json of obs-rust/obs-studio-libs over plain HTTPS (no
# GitHub API), each checked against deps.lock.json's sha256 and installed into
# /opt/obs-deps/<tool> (the prefix it was built for). The CLI is pinned to a
# commit of obs-studio-libs.
#
# This is the whole of the image's libcurl: the AlmaLinux 8 base's 7.61.1
# (2018, EOL) and OpenSSL 1.1.1k (EOL) are not used. The pinned curl 8.23.0 is
# built with --with-ca-fallback, so it uses OpenSSL's default verify paths and
# honours the SSL_CERT_FILE AppRun points at the host's bundle, instead of the
# RHEL-only /etc/pki/tls/certs/ca-bundle.crt (which does not exist on
# Debian/Ubuntu/Arch hosts, obs-rust/obs-studio#1085); its brotli is the pinned
# 1.2.1, not the base's 1.0.6 (CVE-2026-111497 range, #2782); and its TLS is the
# pinned OpenSSL 3.5.9, because curl 8.18+ refuses 1.1.1 (#2774).
COPY build-aux/appimage/manylinux/deps.lock.json /opt/deps/deps.lock.json
RUN uvx --from "git+https://github.com/obs-rust/obs-studio-libs@0c6b4401c001ad858a61b1a7e06d1f8c6be61b6d" \
        obs-studio-libs fetch --lock /opt/deps/deps.lock.json
ENV CMAKE_PREFIX_PATH=/opt/obs-deps/curl:/opt/obs-deps/brotli:/opt/obs-deps/libvpl:/opt/obs-deps/libdatachannel:/opt/obs-deps/qt6:/opt/obs-deps/ffmpeg:/opt/obs-deps/pipewire:/opt/obs-deps/glib:/opt/obs-deps/fdk-aac:/opt/obs-deps/qrcodegen:/opt/obs-deps/headers
ENV PKG_CONFIG_PATH=/opt/obs-deps/curl/lib/pkgconfig:/opt/obs-deps/brotli/lib/pkgconfig:/opt/obs-deps/qt6/lib/pkgconfig:/opt/obs-deps/ffmpeg/lib/pkgconfig:/opt/obs-deps/pipewire/lib/pkgconfig:/opt/obs-deps/glib/lib/pkgconfig:/opt/obs-deps/fdk-aac/lib/pkgconfig:/opt/obs-deps/qrcodegen/lib/pkgconfig:/opt/obs-deps/headers/lib/pkgconfig:/opt/obs-deps/headers/share/pkgconfig
ENV LD_LIBRARY_PATH=/opt/obs-deps/openssl/lib:/opt/obs-deps/curl/lib:/opt/obs-deps/brotli/lib:/opt/obs-deps/libvpl/lib:/opt/obs-deps/libdatachannel/lib:/opt/obs-deps/qt6/lib:/opt/obs-deps/ffmpeg/lib:/opt/obs-deps/pipewire/lib:/opt/obs-deps/glib/lib:/opt/obs-deps/fdk-aac/lib:/opt/obs-deps/qrcodegen/lib
# The pinned curl CLI, not the base's 7.61.1: LD_LIBRARY_PATH already resolves
# libcurl.so.4 to the pinned prefix for every dynamically linked binary, and the
# base's curl tool passes option numbers a libcurl 8 does not know (the rustup
# install step died with `curl: (48)`), so the tool has to be the pinned one too.
ENV PATH=/opt/obs-deps/curl/bin:/opt/obs-deps/glib/bin:/opt/obs-deps/qt6/bin:$PATH

# The three properties the shipped AppImage depends on, asserted here so a
# lock or path edit cannot silently fall back to the base's 7.61.1 / 1.0.6 /
# 1.1.1k: the pkg-config the OBS build sees is the pinned curl, the built
# library links the pinned prefixes, and no RHEL CA path is compiled in.
RUN set -eux \
 && test "$(pkg-config --modversion libcurl)" = "8.23.0" \
 && /opt/obs-deps/curl/bin/curl --version | head -1 \
 && ! grep -a -q '/etc/pki/tls/certs/ca-bundle.crt' /opt/obs-deps/curl/lib/libcurl.so.4 \
 && ldd /opt/obs-deps/curl/lib/libcurl.so.4 | grep -q '/opt/obs-deps/openssl/lib/libssl.so.3' \
 && ldd /opt/obs-deps/curl/lib/libcurl.so.4 | grep -q '/opt/obs-deps/brotli/lib/libbrotli'

# ninja from PyPI (AlmaLinux 8 has 1.8, older than the CMake presets need), and
# SWIG 4 for the Lua scripting bindings (AlmaLinux 8 has 3.0); both build-time only.
# The ubuntu-ci preset's compiler launcher (#957), pinned and checksum-verified;
# this image skips build.py's deps step, which installs it on Ubuntu. The
# installer is a Python script (install_zccache.py); uv is installed above, so
# run it with the manylinux image's own interpreter.
COPY rust/tools/linux-validate/install_zccache.py /tmp/install_zccache.py
RUN /opt/python/cp312-cp312/bin/python3 /tmp/install_zccache.py /usr/local/bin && rm /tmp/install_zccache.py
RUN /opt/python/cp312-cp312/bin/pip install --no-cache-dir ninja==1.11.1.4 swig==4.2.1 \
 && ln -sf /opt/python/cp312-cp312/bin/ninja /usr/local/bin/ninja \
 && ln -sf /opt/python/cp312-cp312/bin/swig /usr/local/bin/swig

# The Rust toolchain pinned in rust-toolchain.toml.
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:$PATH
COPY rust-toolchain.toml /tmp/rust-toolchain.toml
RUN toolchain="$(sed -n 's/^channel = "\(.*\)"/\1/p' /tmp/rust-toolchain.toml)" \
 && curl -sSf --retry 5 --retry-connrefused --retry-delay 5 https://sh.rustup.rs | RUSTUP_MAX_RETRIES=5 sh -s -- -y --profile minimal --default-toolchain "$toolchain" \
 && rm /tmp/rust-toolchain.toml
RUN git config --system --add safe.directory '*'


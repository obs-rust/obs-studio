# glibc 2.28 build environment for the AppImage (obs-rust/obs-studio#926).
# manylinux_2_28 is AlmaLinux 8 (glibc 2.28) with gcc-toolset-14 as the
# default compiler. Libraries AlmaLinux 8 ships new enough come from dnf; the
# rest (Qt 6, FFmpeg and its codecs, PipeWire, GLib, fdk-aac, qrcodegen and
# header-only libraries) are compiled once per version by obs-rust/obs-studio-libs
# (recipes/linux) and fetched here, so this image compiles nothing but OBS.
# The -devel packages also cover what the fetched libraries' .pc files require
# (GIO: libmount; GObject: libffi), so pkg-config can resolve them.
FROM quay.io/pypa/manylinux_2_28_x86_64@sha256:4131ddd7acb9c6c966af02fe7e71edc51e078c2d41f475966697676430bfa2ae

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

# libcurl, rebuilt from AlmaLinux's own source package (same version and
# patches) without the RHEL CA bundle path compiled in (obs-rust/obs-studio#1085).
# The stock build passes /etc/pki/tls/certs/ca-bundle.crt to OpenSSL on every
# handle, so HTTPS failed on hosts without that file (Debian, Ubuntu, Arch, ...).
# Built --with-ca-fallback, libcurl uses OpenSSL's default verify paths, which
# honour the SSL_CERT_FILE that AppRun points at the host's bundle. libcurl does
# not read CURL_CA_BUNDLE (only the curl tool does). gcc-toolset's compiler has no
# annobin plugin, hence --undefine _annotated_build.
RUN dnf install -y -q --setopt=install_weak_deps=False dnf-plugins-core rpm-build \
 && v="$(rpm -q --qf '%{VERSION}-%{RELEASE}' libcurl)" \
 && mkdir /tmp/curl-srpm && cd /tmp/curl-srpm \
 && dnf download -q --source "curl-$v" \
 && dnf builddep -y -q --setopt=install_weak_deps=False "curl-$v.src.rpm" \
 && rpm -i --nosignature "curl-$v.src.rpm" \
 && sed -i 's|--with-ca-bundle=%{_sysconfdir}/pki/tls/certs/ca-bundle.crt|--without-ca-bundle --without-ca-path --with-ca-fallback|' \
        ~/rpmbuild/SPECS/curl.spec \
 && grep -q -- '--with-ca-fallback' ~/rpmbuild/SPECS/curl.spec \
 && rpmbuild -bb --nocheck --undefine _annotated_build ~/rpmbuild/SPECS/curl.spec >/tmp/curl-srpm/build.log 2>&1 \
    || { tail -50 /tmp/curl-srpm/build.log; exit 1; } \
 && r=~/rpmbuild/RPMS/x86_64 \
 && rpm -Uvh --force --nodeps "$r"/libcurl-[0-9]*.rpm "$r"/libcurl-devel-[0-9]*.rpm "$r"/curl-[0-9]*.rpm \
 && ! grep -a -q '/etc/pki/tls/certs/ca-bundle.crt' /usr/lib64/libcurl.so.4 \
 && cd / && rm -rf /tmp/curl-srpm ~/rpmbuild \
 && dnf clean all

# ninja from PyPI (AlmaLinux 8 has 1.8, older than the CMake presets need), and
# SWIG 4 for the Lua scripting bindings (AlmaLinux 8 has 3.0); both build-time only.
# The ubuntu-ci preset's compiler launcher (#957), pinned and checksum-verified;
# this image skips build.py's deps step, which installs it on Ubuntu.
COPY rust/tools/linux-validate/install-zccache.sh /tmp/install-zccache.sh
RUN sh /tmp/install-zccache.sh /usr/local/bin && rm /tmp/install-zccache.sh
RUN /opt/python/cp312-cp312/bin/pip install --no-cache-dir ninja==1.11.1.4 swig==4.2.1 \
 && ln -sf /opt/python/cp312-cp312/bin/ninja /usr/local/bin/ninja \
 && ln -sf /opt/python/cp312-cp312/bin/swig /usr/local/bin/swig

# uv, from obs-rust/obs-studio-libs (the first tool, since it runs the fetch of
# the others), pinned by version and checksum.
ARG UV_VERSION=0.12.13
ARG UV_SHA256=cd646b7421e5e14e749247b29657ae33a8fc4f31294c57b806555c1a7423dfe9
RUN curl -fsSLo /tmp/uv.tar.zst "https://github.com/obs-rust/obs-studio-libs/releases/download/uv-${UV_VERSION}/uv-${UV_VERSION}-linux-x86_64.tar.zst" \
 && echo "${UV_SHA256}  /tmp/uv.tar.zst" | sha256sum -c - \
 && zstd -dc /tmp/uv.tar.zst | tar -xf - -C /usr/local/bin \
 && rm /tmp/uv.tar.zst

# The Rust toolchain pinned in rust-toolchain.toml.
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:$PATH
COPY rust-toolchain.toml /tmp/rust-toolchain.toml
RUN toolchain="$(sed -n 's/^channel = "\(.*\)"/\1/p' /tmp/rust-toolchain.toml)" \
 && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "$toolchain" \
 && rm /tmp/rust-toolchain.toml
RUN git config --system --add safe.directory '*'

# --- prebuilt dependencies -----------------------------------------------------
# From the root manifest.json of obs-rust/obs-studio-libs over plain HTTPS (no
# GitHub API), each checked against deps.lock.json's sha256 and installed into
# /opt/obs-deps/<tool> (the prefix it was built for). The CLI is pinned to a
# commit of obs-studio-libs.
COPY build-aux/appimage/manylinux/deps.lock.json /opt/deps/deps.lock.json
RUN uvx --from "git+https://github.com/obs-rust/obs-studio-libs@b35437f2c34dc2468d4255a88a560073272f14db" \
        obs-studio-libs fetch --lock /opt/deps/deps.lock.json
ENV CMAKE_PREFIX_PATH=/opt/obs-deps/libvpl:/opt/obs-deps/libdatachannel:/opt/obs-deps/qt6:/opt/obs-deps/ffmpeg:/opt/obs-deps/pipewire:/opt/obs-deps/glib:/opt/obs-deps/fdk-aac:/opt/obs-deps/qrcodegen:/opt/obs-deps/headers
ENV PKG_CONFIG_PATH=/opt/obs-deps/qt6/lib/pkgconfig:/opt/obs-deps/ffmpeg/lib/pkgconfig:/opt/obs-deps/pipewire/lib/pkgconfig:/opt/obs-deps/glib/lib/pkgconfig:/opt/obs-deps/fdk-aac/lib/pkgconfig:/opt/obs-deps/qrcodegen/lib/pkgconfig:/opt/obs-deps/headers/lib/pkgconfig:/opt/obs-deps/headers/share/pkgconfig
ENV LD_LIBRARY_PATH=/opt/obs-deps/libvpl/lib:/opt/obs-deps/libdatachannel/lib:/opt/obs-deps/qt6/lib:/opt/obs-deps/ffmpeg/lib:/opt/obs-deps/pipewire/lib:/opt/obs-deps/glib/lib:/opt/obs-deps/fdk-aac/lib:/opt/obs-deps/qrcodegen/lib
ENV PATH=/opt/obs-deps/glib/bin:/opt/obs-deps/qt6/bin:$PATH

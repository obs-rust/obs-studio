use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    // No canonicalize(): on Windows it yields a verbatim `\\?\` path, and
    // MSVC cannot resolve `#include "util/..."` against such an include dir.
    // CARGO_MANIFEST_DIR is already absolute.
    let libobs = manifest
        .ancestors()
        .nth(2)
        .expect("rust/obs-c-oracle has a repo root two levels up")
        .join("libobs");

    let mut oracle = cc::Build::new();
    // Rust never fuses `a * b + c` into an FMA, but GCC/Clang may (clang's
    // default -ffp-contract=on does so on aarch64), which would make float
    // oracles such as vec2_norm (`x*x + y*y`) differ in the last bit from the
    // Rust port. Pin the C oracle to unfused IEEE operations. MSVC does not
    // contract under its default /fp:precise.
    if !oracle.get_compiler().is_like_msvc() {
        oracle.flag("-ffp-contract=off");
        // libobs/util/sse-intrin.h takes SSE from SIMDe outside MSVC. The
        // stand-in keeps `cargo test` free of a SIMDe install.
        oracle.include("oracle/sse-shim");
    }
    oracle
        .file("oracle/bitstream.c")
        .file("oracle/path_extension.c")
        .file("oracle/array_serializer.c")
        .file("oracle/darray.c")
        .file("oracle/crc32.c")
        .file("oracle/vec2.c")
        .file("oracle/nal.c")
        .file("oracle/avc.c")
        .file("oracle/encoder_packet.c")
        .file("oracle/hevc.c")
        .file("oracle/av1.c")
        .file("oracle/graphics_math_axisang.c")
        .file("oracle/graphics_math_bounds.c")
        .file("oracle/graphics_math_math_extra.c")
        .file("oracle/graphics_math_matrix3.c")
        .file("oracle/graphics_math_matrix4.c")
        .file("oracle/graphics_math_plane.c")
        .file("oracle/graphics_math_quat.c")
        .file("oracle/graphics_math_vec3.c")
        .file("oracle/graphics_math_vec4.c")
        .file("oracle/base.c")
        .file("oracle/base_drive.c")
        .file(libobs.join("util/base-variadic.c"))
        .file("oracle/file_serializer.c")
        .file("oracle/file_serializer_host.c")
        .file("oracle/utf8.c")
        .file("oracle/lexer.c")
        .file("oracle/text_lookup.c")
        .file("oracle/dstr.c")
        .file("oracle/dstr_libc.c")
        // dstr-libc.c's conversions: platform.c's verbatim, over oracle/utf8.c.
        .file("oracle/platform_conv_host.c")
        .file("oracle/video_fourcc.c")
        .file("oracle/task.c")
        .file("oracle/profiler_snapshot.c")
        .file("oracle/bmem.c")
        .file("oracle/test_stubs.c")
        .file("oracle/cf_tokenizer.c")
        .file("oracle/video_matrices.c")
        .file("oracle/pipe_args.c");
    // Unix-only: pipe-posix.c needs <spawn.h>.
    if std::env::var_os("CARGO_CFG_UNIX").is_some() {
        oracle.file("oracle/pipe_posix.c");
    }
    oracle
        .include(&libobs)
        // obs.h (for obs-hevc.c and obs-av1.c) needs the CMake-generated obsconfig.h.
        .include("oracle/obsconfig")
        // libobs/util/uthash.h includes <uthash.h>, a system header CMake
        // finds for the real build. The Rust test runners have no uthash
        // package, so the text-lookup oracle uses a vendored copy of the
        // same release (2.3.0, BSD-1-Clause, see vendor/uthash/LICENSE).
        .include(manifest.join("vendor/uthash"))
        .std("c11");
    // task.c calls pthread_mutex_*/pthread_create plus the os_event/os_sem
    // helpers from threading-*.c. On Unix both come from the real sources
    // and the system pthread. MSVC has no pthread: libobs builds against
    // deps/w32-pthreads, so the oracle compiles its single-file build and
    // the real threading-windows.c instead.
    if oracle.get_compiler().is_like_msvc() {
        let deps = libobs.with_file_name("deps");
        oracle
            .include(deps.join("w32-pthreads"))
            .define("PTW32_STATIC_LIB", None)
            .file(deps.join("w32-pthreads/pthread.c"))
            .file(libobs.join("util/threading-windows.c"));
    } else {
        oracle.file(libobs.join("util/threading-posix.c"));
    }
    oracle.compile("obs_c_oracle");

    println!("cargo:rerun-if-changed=oracle");
    println!("cargo:rerun-if-changed=vendor");
    for header in [
        "util/bitstream.c",
        "util/bitstream.h",
        "util/path-extension.c",
        "util/array-serializer.c",
        "util/array-serializer.h",
        "util/darray.h",
        "util/serializer.h",
        "util/bmem.c",
        "util/bmem.h",
        "util/utf8.c",
        "util/utf8.h",
        "util/lexer.c",
        "util/lexer.h",
        "util/text-lookup.c",
        "util/text-lookup.h",
        "util/dstr.c",
        "util/dstr-libc.c",
        "util/dstr.h",
        "util/platform.h",
        "util/base.h",
        "util/crc32.c",
        "util/crc32.h",
        "graphics/vec2.c",
        "graphics/vec2.h",
        "obs-nal.c",
        "obs-nal.h",
        "obs-avc.c",
        "obs-avc.h",
        "obs-hevc.c",
        "obs-hevc.h",
        "obs-av1.c",
        "obs-av1.h",
        "obs-encoder.h",
        "graphics/math-defs.h",
        "graphics/math-extra.h",
        "graphics/math-extra.c",
        "graphics/axisang.c",
        "graphics/axisang.h",
        "graphics/bounds.c",
        "graphics/bounds.h",
        "graphics/matrix3.c",
        "graphics/matrix3.h",
        "graphics/matrix4.c",
        "graphics/matrix4.h",
        "graphics/plane.c",
        "graphics/plane.h",
        "graphics/quat.c",
        "graphics/quat.h",
        "graphics/vec3.c",
        "graphics/vec3.h",
        "graphics/vec4.c",
        "graphics/vec4.h",
        "util/sse-intrin.h",
        "util/base.c",
        "util/base.h",
        "util/base-variadic.c",
        "util/c99defs.h",
        "util/threading.h",
        "util/file-serializer.c",
        "util/file-serializer.h",
        "util/pipe.c",
        "util/pipe.h",
        "util/pipe-posix.c",
        "util/dstr.c",
        "util/dstr.h",
        "util/platform.c",
        "util/utf8.c",
        "util/utf8.h",
        "util/profiler-snapshot.c",
        "util/profiler-snapshot.h",
        "util/profiler.h",
        "util/platform.h",
        "media-io/video-fourcc.c",
        "media-io/video-matrices.c",
        "media-io/video-io.h",
        "media-io/media-io-defs.h",
        "util/task.c",
        "util/task.h",
        "util/deque.h",
        "util/threading-posix.c",
        "util/threading-posix.h",
        "util/threading-windows.c",
        "util/threading-windows.h",
        "util/cf-tokenizer.c",
        "util/cf-lexer.h",
    ] {
        println!("cargo:rerun-if-changed={}", libobs.join(header).display());
    }
}

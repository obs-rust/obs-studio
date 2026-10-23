use std::path::{Path, PathBuf};

/// FNV-1a 64-bit over the bytes that change what the oracle archive contains:
/// this script, every *.c, *.h, and *.m under `oracle/`, `vendor/`, `libobs/`
/// and `deps/w32-pthreads/`, and the flag list. The whole of libobs is hashed
/// because oracle wrappers `#include` libobs sources and headers directly, so
/// any libobs edit can change the compiled archive. zccache keys an rlib on
/// argv + Rust inputs but never hashes the native archive a build script
/// bundles, so without this an oracle-affecting change can be served a stale
/// cached rlib (zackees/zccache#1945). `env!()` in src/lib.rs turns the value
/// into a rustc `env-dep`, which zccache does fold into the artifact key.
/// No new dependency: a simple stable hash is enough for change detection.
fn oracle_input_hash(roots: &[PathBuf], flags: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    let hash_file = |path: &Path, mix: &mut dyn FnMut(&[u8])| {
        mix(path.to_string_lossy().as_bytes());
        mix(&std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display())));
    };
    hash_file(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("build.rs")
            .as_path(),
        &mut mix,
    );
    let mut hash_inputs: Vec<PathBuf> = Vec::new();
    for root in roots {
        collect_c_sources(root, &mut hash_inputs);
    }
    hash_inputs.sort();
    for path in &hash_inputs {
        hash_file(path, &mut mix);
    }
    for flag in flags {
        mix(flag.as_bytes());
    }
    h
}

fn collect_c_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    if !dir.is_dir() {
        return;
    }
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_c_sources(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e == "c" || e == "h" || e == "m")
        {
            out.push(path);
        }
    }
}

/// Copies `[start line, end line]` (inclusive, matched by prefix) out of an
/// original plugin source, verbatim. Used where the whole file cannot be
/// compiled without a GPU SDK (plugins/obs-nvenc needs nv-codec-headers): the
/// oracle compiles exactly the original lines it needs. Panics if the markers
/// move, so a C change that breaks the extraction fails the build.
fn extract_c(src: &Path, start: &str, end: &str) -> String {
    let text =
        std::fs::read_to_string(src).unwrap_or_else(|e| panic!("read {}: {e}", src.display()));
    let lines: Vec<&str> = text.lines().collect();
    let first = lines
        .iter()
        .position(|l| l.starts_with(start))
        .unwrap_or_else(|| panic!("{}: no line starting with {start:?}", src.display()));
    let last = lines[first..]
        .iter()
        .position(|l| l.starts_with(end))
        .map(|i| first + i)
        .unwrap_or_else(|| panic!("{}: no {end:?} after {start:?}", src.display()));
    let mut out = format!(
        "#line {} \"{}\"\n",
        first + 1,
        src.display().to_string().replace('\\', "/")
    );
    for l in &lines[first..=last] {
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// Keep an implementation verbatim after its native build PCH. The oracle
/// supplies the unit's own headers instead of unrelated libobs dependencies
/// (notably zlib, which the Windows Rust test runners do not install).
fn extract_after_libobs_pch(src: &Path) -> String {
    let text = std::fs::read_to_string(src).expect("read lifecycle implementation");
    let (prefix, body) = text
        .split_once("#include \"libobs/build/libobs.pch.h\"")
        .expect("lifecycle implementation must include its libobs PCH");
    let body = body
        .strip_prefix("\r\n")
        .or_else(|| body.strip_prefix('\n'))
        .expect("libobs PCH include must occupy its own line");
    format!(
        "#line {} \"{}\"\n{body}",
        prefix.lines().count() + 2,
        src.display().to_string().replace('\\', "/")
    )
}
/// The strict GCC/Clang warnings enabled so far (obs-rust/obs-studio#516).
/// Add a flag once every target builds clean under it.
const STRICT_FLAGS: &[&str] = &[
    "-Wvla",
    "-Wshadow",
    "-Wundef",
    "-Wdouble-promotion",
    "-Wstrict-prototypes",
    "-Wformat-nonliteral",
    "-Wpedantic",
    "-Wcast-qual",
    "-Wsign-conversion",
    "-Wmissing-prototypes",
];

/// On windows-gnu, libgcc's emutls (`__emutls_get_address`) calls the POSIX
/// pthread symbols and treats `pthread_key_t` as a 32-bit unsigned long.
/// w32-pthreads, which the oracle links in one object, uses a pointer-sized
/// `pthread_key_t`. High-entropy ASLR then makes the truncated key fault
/// (`STATUS_ACCESS_VIOLATION` in `pthread_getspecific`, obs-rust/obs-studio#441).
/// Wine and a debugger heap sit in the low 4GB, so the truncated pointer still
/// works there. Renaming the oracle's copies leaves the unprefixed symbols for
/// libwinpthread. MSVC has no libgcc, so it keeps the original names.
macro_rules! gnu_oracle_renames {
    ($($name:ident),* $(,)?) => {
        &[$((stringify!($name), concat!("oracle_", stringify!($name)))),*]
    };
}

const GNU_ORACLE_RENAMES: &[(&str, &str)] = gnu_oracle_renames!(
    pthread_attr_destroy,
    pthread_attr_getdetachstate,
    pthread_attr_getinheritsched,
    pthread_attr_getschedparam,
    pthread_attr_getschedpolicy,
    pthread_attr_getscope,
    pthread_attr_getstackaddr,
    pthread_attr_getstacksize,
    pthread_attr_init,
    pthread_attr_setdetachstate,
    pthread_attr_setinheritsched,
    pthread_attr_setschedparam,
    pthread_attr_setschedpolicy,
    pthread_attr_setscope,
    pthread_attr_setstackaddr,
    pthread_attr_setstacksize,
    pthread_barrier_destroy,
    pthread_barrier_init,
    pthread_barrier_wait,
    pthread_barrierattr_destroy,
    pthread_barrierattr_getpshared,
    pthread_barrierattr_init,
    pthread_barrierattr_setpshared,
    pthread_cancel,
    pthread_cond_broadcast,
    pthread_cond_destroy,
    pthread_cond_init,
    pthread_cond_signal,
    pthread_cond_timedwait,
    pthread_cond_wait,
    pthread_condattr_destroy,
    pthread_condattr_getpshared,
    pthread_condattr_init,
    pthread_condattr_setpshared,
    pthread_create,
    pthread_delay_np,
    pthread_detach,
    pthread_equal,
    pthread_exit,
    pthread_getconcurrency,
    pthread_getschedparam,
    pthread_getspecific,
    pthread_getunique_np,
    pthread_getw32threadhandle_np,
    pthread_getw32threadid_np,
    pthread_join,
    pthread_key_create,
    pthread_key_delete,
    pthread_kill,
    pthread_mutex_consistent,
    pthread_mutex_destroy,
    pthread_mutex_init,
    pthread_mutex_lock,
    pthread_mutex_timedlock,
    pthread_mutex_trylock,
    pthread_mutex_unlock,
    pthread_mutexattr_destroy,
    pthread_mutexattr_getkind_np,
    pthread_mutexattr_getpshared,
    pthread_mutexattr_getrobust,
    pthread_mutexattr_gettype,
    pthread_mutexattr_init,
    pthread_mutexattr_setkind_np,
    pthread_mutexattr_setpshared,
    pthread_mutexattr_setrobust,
    pthread_mutexattr_settype,
    pthread_num_processors_np,
    pthread_once,
    pthread_rwlock_destroy,
    pthread_rwlock_init,
    pthread_rwlock_rdlock,
    pthread_rwlock_timedrdlock,
    pthread_rwlock_timedwrlock,
    pthread_rwlock_tryrdlock,
    pthread_rwlock_trywrlock,
    pthread_rwlock_unlock,
    pthread_rwlock_wrlock,
    pthread_rwlockattr_destroy,
    pthread_rwlockattr_getpshared,
    pthread_rwlockattr_init,
    pthread_rwlockattr_setpshared,
    pthread_self,
    pthread_setcancelstate,
    pthread_setcanceltype,
    pthread_setconcurrency,
    pthread_setschedparam,
    pthread_setspecific,
    pthread_spin_destroy,
    pthread_spin_init,
    pthread_spin_lock,
    pthread_spin_trylock,
    pthread_spin_unlock,
    pthread_testcancel,
    pthread_timechange_handler_np,
    pthread_win32_process_attach_np,
    pthread_win32_process_detach_np,
    pthread_win32_test_features_np,
    pthread_win32_thread_attach_np,
    pthread_win32_thread_detach_np,
    sched_get_priority_max,
    sched_get_priority_min,
    sched_getscheduler,
    sched_setscheduler,
    sched_yield,
    sem_close,
    sem_destroy,
    sem_getvalue,
    sem_init,
    sem_open,
    sem_post,
    sem_post_multiple,
    sem_timedwait,
    sem_trywait,
    sem_unlink,
    sem_wait,
);

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

    let unix = std::env::var_os("CARGO_CFG_UNIX").is_some();
    let windows = std::env::var_os("CARGO_CFG_WINDOWS").is_some();
    let gio = if unix && std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() != Some("macos") {
        pkg_config::Config::new().probe("gio-2.0").ok()
    } else {
        None
    };
    let mut oracle = cc::Build::new();
    // Rust never fuses `a * b + c` into an FMA, but GCC/Clang may (clang's
    // default -ffp-contract=on does so on aarch64), which would make float
    // oracles such as vec2_norm (`x*x + y*y`) differ in the last bit from the
    // Rust port. Pin the C oracle to unfused IEEE operations. MSVC does not
    // contract under its default /fp:precise.
    let mut flags: Vec<&str> = Vec::new();
    // Owned -D flags for the input hash. build.rs is hashed too; these keep
    // the flag list complete when the rename set changes beside the script.
    let mut owned_flags: Vec<String> = Vec::new();
    // Warnings are errors (obs-rust/obs-studio#516): cc maps these to
    // -Wall -Wextra -Werror, or /W4 /WX for MSVC and clang-cl. STRICT_FLAGS
    // is the ratchet toward the full strict set: a flag joins once every
    // target is clean under it (#516 lists the remaining ones).
    oracle
        .warnings(true)
        .extra_warnings(true)
        .warnings_into_errors(true);
    flags.extend(["-Wall", "-Wextra", "-Werror"]);
    if !oracle.get_compiler().is_like_msvc() {
        for flag in STRICT_FLAGS {
            oracle.flag(flag);
            flags.push(flag);
        }
    }
    if oracle.get_compiler().is_like_msvc() {
        // As the shipped libobs build (cmake/windows/compilerconfig.cmake).
        oracle.define("_CRT_SECURE_NO_WARNINGS", None);
        flags.push("-D_CRT_SECURE_NO_WARNINGS");
        // /W4 is stricter than the shipped /W3. C4702 (unreachable code)
        // only fires inside the vendored uthash HASH_* macros, whose
        // uthash_fatal path does not return. C4127 is the same macros'
        // `do { ... } while (0)`.
        oracle.flag("/wd4702");
        flags.push("/wd4702");
        oracle.flag("/wd4127");
        flags.push("/wd4127");
        // obs-internal.h uses a C11 flexible array for caption_text.text.
        // MSVC diagnoses it as extension C4200 even in C11 mode (#1427).
        oracle.flag("/wd4200");
        flags.push("/wd4200");
    }
    if oracle.get_compiler().is_like_clang() {
        // soldr's Apple toolchains pass -fuse-ld=lld in CFLAGS, unused by a
        // compile-only invocation; keep the warning, not the error.
        oracle.flag("-Wno-error=unused-command-line-argument");
        flags.push("-Wno-error=unused-command-line-argument");
    }
    let windows_gnu =
        windows && std::env::var("CARGO_CFG_TARGET_ENV").ok().as_deref() == Some("gnu");
    if windows_gnu {
        // audio-io.c includes <Windows.h>. MinGW's header is windows.h, and
        // a case-sensitive host (Ubuntu's windows-gnu link) will not find
        // the capital name. This directory is first, so the wrapper is the
        // one that gets included.
        oracle.include("oracle/mingw-case");
        // C99 printf (%zu) from MinGW's stdio, matching PRINTFATTR's
        // __gnu_printf__ archetype there.
        oracle.define("__USE_MINGW_ANSI_STDIO", Some("1"));
        flags.push("-D__USE_MINGW_ANSI_STDIO=1");
        // GCC 15 rejects `struct stat` in platform.h's prototype unless a
        // previous include has declared it. Force that for the oracle only.
        oracle.flag("-include").flag("sys/stat.h");
        owned_flags.push("-include sys/stat.h".to_string());
        for (from, to) in GNU_ORACLE_RENAMES {
            oracle.define(from, Some(*to));
            owned_flags.push(format!("-D{from}={to}"));
        }
        // libgcc's unprefixed pthread_* must resolve to MinGW winpthreads.
        println!("cargo:rustc-link-lib=pthread");
    }
    if !oracle.get_compiler().is_like_msvc() {
        oracle.flag("-ffp-contract=off");
        flags.push("-ffp-contract=off");
        // libobs is built with -fno-strict-aliasing (cmake/common/compiler_common.cmake,
        // cmake/linux/compilerconfig.cmake); the oracle must match the shipped C.
        oracle.flag("-fno-strict-aliasing");
        flags.push("-fno-strict-aliasing");
        // libobs/util/sse-intrin.h takes SSE from SIMDe outside MSVC. The
        // stand-in keeps `cargo test` free of a SIMDe install.
        oracle.include("oracle/sse-shim");
    }
    // Every oracle/*.c is compiled (obs-rust/obs-studio#137): a port adds its
    // wrapper file, not a line here. `*_posix.c` wrappers are Unix-only
    // (pipe-posix.c needs <spawn.h>) and `*_windows.c` wrappers are
    // Windows-only (pipe-windows.c needs <windows.h>).
    let mut sources: Vec<PathBuf> = std::fs::read_dir(manifest.join("oracle"))
        .expect("read oracle/")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "c"))
        .filter(|p| unix || !p.to_string_lossy().ends_with("_posix.c"))
        .filter(|p| windows || !p.to_string_lossy().ends_with("_windows.c"))
        // The D-Bus and portal oracles need GIO headers. Keep other oracle
        // families testable on hosts where GIO is not installed.
        .filter(|p| {
            gio.is_some()
                || p.file_name()
                    .is_none_or(|n| n != "dbus_inhibit_posix.c" && n != "portal_inhibit_posix.c")
        })
        // The original platform-windows.c has only ever been an MSVC
        // translation unit; under MinGW -Wall -Werror (#516) it fails on
        // headers and format checks, not on anything the port compares.
        // Its oracle and the tests that use it are MSVC-only (#707).
        .filter(|p| !(windows_gnu && p.file_name().is_some_and(|n| n == "platform_windows.c")))
        .collect();
    sources.sort();
    oracle.files(sources);
    if let Some(gio) = &gio {
        for include in &gio.include_paths {
            oracle.include(include);
        }
    }
    // Keep the oracle's threading helpers private. The Rust obs-util crate
    // exports the real libobs os_event/os_sem/os_set_thread_name symbols;
    // the C oracle must call its renamed copy so both implementations can link
    // into the same test executable. Unix includes them from
    // oracle/threading_posix.c, Windows from oracle/threading_windows.c.
    if unix || windows {
        for (from, to) in [
            ("os_event_init", "oracle_os_event_init"),
            ("os_event_destroy", "oracle_os_event_destroy"),
            ("os_event_wait", "oracle_os_event_wait"),
            ("os_event_timedwait", "oracle_os_event_timedwait"),
            ("os_event_try", "oracle_os_event_try"),
            ("os_event_signal", "oracle_os_event_signal"),
            ("os_event_reset", "oracle_os_event_reset"),
            ("os_sem_init", "oracle_os_sem_init"),
            ("os_sem_destroy", "oracle_os_sem_destroy"),
            ("os_sem_post", "oracle_os_sem_post"),
            ("os_sem_wait", "oracle_os_sem_wait"),
            ("os_set_thread_name", "oracle_os_set_thread_name"),
        ] {
            oracle.define(from, Some(to));
        }
    }
    // plugins/obs-nvenc loader parity (obs-rust/obs-studio#520): the
    // original load_cuda_lib(), cuda_functions[] and load_nvenc_lib(),
    // extracted verbatim for oracle/obs_nvenc_loader.c.
    let nvenc_dir = libobs.with_file_name("plugins").join("obs-nvenc");
    let extract_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("obs-nvenc-extract");
    std::fs::create_dir_all(&extract_dir).expect("create obs-nvenc-extract");
    let cuda_load = nvenc_dir.join("cuda-load.c.h");
    let nvenc_load = nvenc_dir.join("nvenc-load.c.h");
    for (name, src, start, end) in [
        (
            "load_cuda_lib.inc",
            &cuda_load,
            "bool load_cuda_lib(void)",
            "}",
        ),
        (
            "cuda_functions.inc",
            &cuda_load,
            "typedef struct cuda_function {",
            "static const size_t num_cuda_funcs",
        ),
        (
            "load_nvenc_lib.inc",
            &nvenc_load,
            "bool load_nvenc_lib(void)",
            "}",
        ),
    ] {
        std::fs::write(extract_dir.join(name), extract_c(src, start, end)).expect("write extract");
    }
    for stem in ["effect-parser-lifecycle", "effect-parser-lifecycle-adapter"] {
        let src = libobs.join("graphics").join(format!("{stem}.c.h"));
        std::fs::write(
            extract_dir.join(format!("{stem}.inc")),
            extract_after_libobs_pch(&src),
        )
        .expect("write lifecycle extract");
    }
    oracle.include(&extract_dir);

    oracle
        .include(&libobs)
        // The libobs implementation files include their precompiled header by
        // its repo-root path ("libobs/build/...", BUILD.md).
        .include(libobs.parent().expect("libobs has a parent"))
        // obs.h (for obs-hevc.c, obs-av1.c and obs-avc.c) needs the CMake-generated obsconfig.h.
        .include("oracle/obsconfig")
        // libobs/util/uthash.h includes <uthash.h>, a system header CMake
        // finds for the real build. The Rust test runners have no uthash
        // package, so the text-lookup oracle uses a vendored copy of the
        // same release (2.3.0, BSD-1-Clause, see vendor/uthash/LICENSE).
        .include(manifest.join("vendor/uthash"))
        // obs-properties.c includes obs-internal.h, which includes
        // <caption/caption.h> from deps/libcaption (headers only).
        .include(libobs.with_file_name("deps").join("libcaption"))
        // pulseaudio-stream-flags.h includes <pulse/def.h>. The stand-in
        // forwards to libpulse's header where one is installed and defines
        // the values the oracle needs where not (oracle/pulse-shim/pulse/def.h).
        .include(manifest.join("oracle/pulse-shim"))
        // util/profiler-io.c includes <zlib.h>, which the Windows Rust test
        // runners do not have. The stand-in writes the "gzip" stream
        // uncompressed (oracle/zlib-shim/zlib.h), so Tier 3 compares it with
        // the Rust port's decompressed output.
        .include(manifest.join("oracle/zlib-shim"))
        .std("c11");
    // The real build is gnu17, whose glibc headers see _DEFAULT_SOURCE (POSIX
    // 2008: PTHREAD_MUTEX_RECURSIVE for util/threading.h). Strict c11 hides
    // it, and a unity group's precompiled header (BUILD.md) brings
    // threading.h to every oracle file of its family, not only the ones that
    // used to include it.
    if unix {
        oracle.define("_DEFAULT_SOURCE", None);
    }
    // task.c calls pthread_mutex_*/pthread_create plus os_event/os_sem helpers.
    // Those helpers come from oracle/threading_posix.c or
    // oracle/threading_windows.c, included under private names. MSVC has no
    // pthread: libobs builds against deps/w32-pthreads, so the oracle compiles
    // its single-file build. Chosen by target, not compiler: GCC/Clang
    // targeting Windows (MinGW) has no system pthread either
    // (obs-rust/obs-studio#291).
    if windows {
        let deps = libobs.with_file_name("deps");
        oracle
            .include(deps.join("w32-pthreads"))
            .define("PTW32_STATIC_LIB", None);
        flags.push("-DPTW32_STATIC_LIB");
        // Vendored third-party code: its own library, warnings off (#516).
        let mut pthreads = cc::Build::new();
        pthreads
            .include(deps.join("w32-pthreads"))
            .define("PTW32_STATIC_LIB", None)
            .file(deps.join("w32-pthreads/pthread.c"))
            .warnings(false)
            .std("c11");
        if windows_gnu {
            for (from, to) in GNU_ORACLE_RENAMES {
                pthreads.define(from, Some(*to));
            }
        }
        pthreads.compile("obs_c_oracle_w32_pthreads");
        // platform-windows.c calls these. kernel32 is the default.
        for lib in [
            "shell32", "ole32", "advapi32", "psapi", "rpcrt4", "winmm", "avrt",
        ] {
            println!("cargo:rustc-link-lib={lib}");
        }
    }
    // Every directory the oracle can read C/headers from. libobs is the
    // important one: wrappers include its sources directly, so a libobs-only
    // change must change the hash. w32-pthreads only exists on Windows
    // checkouts; collect_c_sources tolerates its absence.
    let roots = [
        manifest.join("oracle"),
        manifest.join("vendor"),
        libobs.clone(),
        libobs.with_file_name("deps").join("w32-pthreads"),
        nvenc_dir.clone(),
    ];
    // platform-cocoa.m is Objective-C and returns NULL from a bool
    // (cfstr_copy_dstr). It is a separate cc build so that one warning
    // does not have to be disabled for every other oracle file.
    let macos = std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() == Some("macos");
    if macos {
        owned_flags.push("-fobjc-arc".to_string());
        owned_flags.push("-Wno-int-conversion".to_string());
        // The original file stores a sysctl result it never reads.
        owned_flags.push("-Wno-unused-but-set-variable".to_string());
        // soldr appends CFLAGS after the builder flags and restates
        // -mmacosx-version-min=10.12, so a version-min here does not
        // win. The file uses NSURL keys from 10.13. The project target
        // is 13.0 (CMakePresets.json). Silence that one cross warning.
        owned_flags.push("-Wno-unguarded-availability-new".to_string());
    }
    // obs-data.c calls its own deprecated autoselect setters. The shipped
    // libobs build silences that on this one file; the oracle has to as well
    // or -Werror fails the include of the original source.
    if oracle.get_compiler().is_like_msvc() {
        oracle.flag("/wd4996");
        owned_flags.push("/wd4996".into());
    } else {
        oracle.flag("-Wno-deprecated-declarations");
        owned_flags.push("-Wno-deprecated-declarations".into());
    }
    // obs-properties.c initializes `struct list_item item = {NULL};`; the
    // shipped build tolerates it, the oracle's -Werror does not.
    if !oracle.get_compiler().is_like_msvc() || oracle.get_compiler().is_like_clang_cl() {
        oracle.flag("-Wno-missing-field-initializers");
        owned_flags.push("-Wno-missing-field-initializers".into());
    }
    let jansson_src = manifest.join("vendor/jansson/src");
    oracle.include(&jansson_src);
    owned_flags.push("-Ivendor/jansson/src".into());
    let mut hash_flags: Vec<&str> = flags.clone();
    hash_flags.extend(owned_flags.iter().map(String::as_str));
    println!(
        "cargo:rustc-env=OBS_ORACLE_INPUT_HASH={:016x}",
        oracle_input_hash(&roots, &hash_flags)
    );
    // platform-nix.c calls uuid_generate. macOS has it in libSystem.
    // Linux needs libuuid, which the OBS build already links.
    // FreeBSD also calls libsysinfo's sysinfo() for the total-memory path,
    // and the uuid header lives in /usr/local.
    match std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() {
        Some("linux") => println!("cargo:rustc-link-lib=uuid"),
        Some("freebsd") => {
            oracle.include("/usr/local/include");
            println!("cargo:rustc-link-search=native=/usr/local/lib");
            println!("cargo:rustc-link-lib=uuid");
            println!("cargo:rustc-link-lib=sysinfo");
        }
        _ => {}
    }
    oracle.compile("obs_c_oracle");

    // Jansson 2.14.1 for cargo test. libobs links its own copy, so this
    // staticlib stays on the oracle and is not a rustc-link-lib of obs-data.
    // Warnings stay off: the third-party sources are not part of the ratchet.
    let mut jansson = cc::Build::new();
    jansson
        .include(&jansson_src)
        .warnings(false)
        .define("HAVE_CONFIG_H", None);
    for name in [
        "dump.c",
        "error.c",
        "hashtable.c",
        "hashtable_seed.c",
        "load.c",
        "memory.c",
        "pack_unpack.c",
        "strbuffer.c",
        "strconv.c",
        "utf.c",
        "value.c",
        "version.c",
    ] {
        jansson.file(jansson_src.join(name));
    }
    jansson.compile("jansson_vendor");

    if macos {
        let mut objc = cc::Build::new();
        objc.warnings(true)
            .extra_warnings(true)
            .warnings_into_errors(true)
            .flag("-fobjc-arc")
            .flag("-Wno-int-conversion")
            .flag("-Wno-unused-but-set-variable")
            .flag("-Wno-error=unused-command-line-argument")
            .flag("-Wno-unguarded-availability-new")
            .include(&libobs)
            .include(libobs.parent().expect("libobs has a parent"))
            .include("oracle/obsconfig")
            .file(manifest.join("oracle/platform_cocoa.m"));
        objc.compile("obs_c_oracle_cocoa");
        for framework in ["Foundation", "CoreFoundation", "IOKit", "Cocoa"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }

    println!("cargo:rerun-if-changed=oracle");
    println!("cargo:rerun-if-changed=vendor");
    // Cargo scans a watched directory recursively, so any libobs source or
    // header an oracle includes reruns this script.
    println!("cargo:rerun-if-changed={}", libobs.display());
    println!("cargo:rerun-if-changed={}", cuda_load.display());
    println!("cargo:rerun-if-changed={}", nvenc_load.display());
}

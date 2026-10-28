//! Explicit Windows SDK/dependency integration in the existing application harness.
use obs_build::{NativeBuild, SharedLibrary, graphics, libobs};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one block per build step of the canonical manifest"
)]
fn core_builds_from_the_canonical_manifest() {
    let Some(dependencies) = std::env::var_os("OBS_BUILD_WINDOWS_DEPENDENCIES") else {
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: std::env::var("OBS_BUILD_WINDOWS_TARGET")
            .unwrap_or_else(|_| obs_build::BUILD_TARGET.into()),
        output: std::env::var_os("OBS_BUILD_WINDOWS_OUTPUT")
            .expect("set OBS_BUILD_WINDOWS_OUTPUT")
            .into(),
    };
    assert!(build.target.contains("windows-msvc"));
    let options = libobs::Options {
        native_dependencies: Some(dependencies.into()),
        hevc: std::env::var_os("OBS_BUILD_DISABLE_HEVC").is_none(),
        unity: std::env::var_os("OBS_BUILD_WINDOWS_SEPARATE").is_none(),
        pch: std::env::var_os("OBS_BUILD_WINDOWS_PCH").is_some(),
        ..libobs::Options::default()
    };
    let (library, reference_library) = if let Some(archive) =
        std::env::var_os("OBS_BUILD_WINDOWS_RUST_ARCHIVE")
    {
        let pair = libobs::build_pair(&build, &root, &options, &PathBuf::from(archive)).unwrap();
        let reference = exports(&pair.reference);
        assert_eq!(reference, exports(&pair.with_rust));
        if let Some(original) = std::env::var_os("OBS_BUILD_WINDOWS_REFERENCE") {
            assert_eq!(
                reference,
                exports(Path::new(&original)),
                "Cargo must preserve the actual CMake reference exports"
            );
        }
        std::fs::write(
            build.output.join("windows-symbols.json"),
            serde_json::to_vec_pretty(&reference).unwrap(),
        )
        .unwrap();
        (pair.with_rust, Some(pair.reference))
    } else {
        (
            libobs::build_c_reference_with_options(&build, &root, &options).unwrap(),
            None,
        )
    };
    assert_eq!(library.file_name().unwrap(), "obs.dll");
    assert!(library.is_file());
    assert!(
        library.with_extension("lib").is_file(),
        "the public DLL needs an import library"
    );
    assert!(!exports(&library).is_empty());
    let mut renderers = serde_json::Map::new();
    let mut graphics_probe = None;
    if std::env::var_os("OBS_BUILD_WINDOWS_GRAPHICS").is_some() {
        let gpu_priority = std::env::var("OBS_BUILD_WINDOWS_GPU_PRIORITY")
            .map(|value| {
                value
                    .parse::<i32>()
                    .expect("GPU priority must be an integer")
            })
            .unwrap_or(0);
        for (name, renderer) in [
            (
                "D3D11",
                graphics::build_d3d11(&build, &root, &library, &options, gpu_priority).unwrap(),
            ),
            (
                "OPENGL",
                graphics::build_opengl(&build, &root, &library, &options).unwrap(),
            ),
        ] {
            let symbols = exports(&renderer);
            assert!(
                symbols.contains("device_create"),
                "{name} needs its public device API"
            );
            assert!(symbols.contains("device_destroy"));
            if let Some(reference) = std::env::var_os(format!("OBS_BUILD_WINDOWS_{name}_REFERENCE"))
            {
                assert_eq!(
                    symbols,
                    exports(Path::new(&reference)),
                    "{name} exports differ from CMake"
                );
            }
            renderers.insert(name.into(), serde_json::json!(renderer));
        }
        graphics_probe = Some(renderer_probe(&build, &root, &options, &library));
    }
    std::fs::write(
        build.output.join("windows-core-path.txt"),
        library.to_str().unwrap(),
    )
    .unwrap();
    let probe = loader_probe(&build, &root, &options, &library);
    let pthreads = build.output.join("w32-pthreads/w32-pthreads.dll");
    assert!(pthreads.is_file());
    std::fs::write(
        build.output.join("windows-artifacts.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "core": library, "reference": reference_library,
            "pthreads": dunce::canonicalize(pthreads).unwrap(), "loader_probe": probe,
            "target": build.target, "unity": options.unity, "pch": options.pch, "hevc": options.hevc,
            "renderers": renderers, "graphics_probe": graphics_probe,
        }))
        .unwrap(),
    )
    .unwrap();
}

fn renderer_probe(
    build: &NativeBuild,
    root: &Path,
    options: &libobs::Options,
    core: &Path,
) -> PathBuf {
    let source = build.output.join("config/windows-renderer-probe.c");
    std::fs::write(&source, r#"
#include <obs.h>
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

static int same_file(const wchar_t *left, const wchar_t *right) {
    HANDLE a = CreateFileW(left, FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_EXISTING, 0, NULL);
    HANDLE b = CreateFileW(right, FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_EXISTING, 0, NULL);
    BY_HANDLE_FILE_INFORMATION ai, bi;
    int equal = a != INVALID_HANDLE_VALUE && b != INVALID_HANDLE_VALUE &&
        GetFileInformationByHandle(a, &ai) && GetFileInformationByHandle(b, &bi) &&
        ai.dwVolumeSerialNumber == bi.dwVolumeSerialNumber &&
        ai.nFileIndexHigh == bi.nFileIndexHigh && ai.nFileIndexLow == bi.nFileIndexLow;
    if (a != INVALID_HANDLE_VALUE) CloseHandle(a);
    if (b != INVALID_HANDLE_VALUE) CloseHandle(b);
    return equal;
}

int wmain(int argc, wchar_t **argv) {
    if (argc != 3) return 10;
    if (obs_get_version() != LIBOBS_API_VER) return 11;
    bool d3d = wcscmp(argv[2], L"D3D11") == 0;
    if (!d3d && wcscmp(argv[2], L"OPENGL")) return 12;
    HMODULE renderer = LoadLibraryW(argv[1]);
    if (!renderer) { wprintf(L"LoadLibrary failed: %lu\n", GetLastError()); return 13; }
    typedef const char *(*name_fn)(void);
    typedef int (*type_fn)(void);
    FARPROC name_export = GetProcAddress(renderer, "device_get_name");
    type_fn type = (type_fn)GetProcAddress(renderer, "device_get_type");
    name_fn name = (name_fn)name_export;
    if (!name || !type || strcmp(name(), d3d ? "Direct3D 11" : "OpenGL") ||
        type() != (d3d ? GS_DEVICE_DIRECT3D_11 : GS_DEVICE_OPENGL)) return 14;
    HMODULE owner = NULL;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
        GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, (LPCWSTR)name_export, &owner)) return 15;
    wchar_t loaded[32768];
    DWORD size = GetModuleFileNameW(owner, loaded, 32768);
    if (!size || size >= 32768 || !same_file(loaded, argv[1])) return 16;
    DWORD ignored = 0, bytes = GetFileVersionInfoSizeW(loaded, &ignored);
    if (!bytes) return 17;
    void *resource = malloc(bytes);
    if (!resource || !GetFileVersionInfoW(loaded, 0, bytes, resource)) return 18;
    VS_FIXEDFILEINFO *version = NULL;
    UINT length = 0;
    if (!VerQueryValueW(resource, L"\\", (void **)&version, &length) ||
        length < sizeof(*version) || version->dwSignature != VS_FFI_SIGNATURE) return 19;
    wchar_t *description = NULL;
    if (!VerQueryValueW(resource, L"\\StringFileInfo\\040904B0\\FileDescription", (void **)&description, &length) ||
        !length || wcscmp(description, d3d ? L"OBS Library D3D11 wrapper" : L"OBS Library OpenGL wrapper")) return 20;
    wprintf(L"renderer: %ls; description: %ls; version: %u.%u.%u.%u\n", loaded, description,
        HIWORD(version->dwFileVersionMS), LOWORD(version->dwFileVersionMS),
        HIWORD(version->dwFileVersionLS), LOWORD(version->dwFileVersionLS));
    free(resource);
    FreeLibrary(renderer);
    return 0;
}
"#).unwrap();
    build
        .executable(&SharedLibrary {
            name: "windows-renderer-probe".into(),
            sources: vec![source],
            include_dirs: vec![
                root.into(),
                root.join("libobs"),
                root.join("deps/w32-pthreads"),
                build.output.join("config"),
                options
                    .native_dependencies
                    .as_ref()
                    .unwrap()
                    .join("include"),
            ],
            dependencies: vec![core.into()],
            compile_options: vec!["/utf-8".into()],
            c_compile_options: vec!["/std:c17".into()],
            link_options: vec!["kernel32.lib".into(), "version.lib".into()],
            ..SharedLibrary::default()
        })
        .unwrap()
}

fn loader_probe(
    build: &NativeBuild,
    root: &Path,
    options: &libobs::Options,
    core: &Path,
) -> PathBuf {
    let source = build.output.join("config/windows-core-loader-probe.c");
    std::fs::write(
        &source,
        r#"
#include <obs.h>
#include <windows.h>
#include <stdio.h>

static int same_file(const wchar_t *left, const wchar_t *right) {
    HANDLE a = CreateFileW(left, FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_EXISTING, 0, NULL);
    HANDLE b = CreateFileW(right, FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_EXISTING, 0, NULL);
    BY_HANDLE_FILE_INFORMATION ai, bi;
    int equal = a != INVALID_HANDLE_VALUE && b != INVALID_HANDLE_VALUE &&
        GetFileInformationByHandle(a, &ai) && GetFileInformationByHandle(b, &bi) &&
        ai.dwVolumeSerialNumber == bi.dwVolumeSerialNumber &&
        ai.nFileIndexHigh == bi.nFileIndexHigh && ai.nFileIndexLow == bi.nFileIndexLow;
    if (a != INVALID_HANDLE_VALUE) CloseHandle(a);
    if (b != INVALID_HANDLE_VALUE) CloseHandle(b);
    return equal;
}

int wmain(int argc, wchar_t **argv) {
    if (argc != 2) return 10;
    if (obs_get_version() != LIBOBS_API_VER) return 11;
    HMODULE core = GetModuleHandleW(L"obs.dll");
    FARPROC version = core ? GetProcAddress(core, "obs_get_version") : NULL;
    if (!version) return 12;
    HMODULE owner = NULL;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
        GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, (LPCWSTR)version, &owner)) return 12;
    wchar_t loaded[32768];
    DWORD size = GetModuleFileNameW(owner, loaded, 32768);
    if (!size || size >= 32768) return 13;
    wprintf(L"loaded core: %ls\n", loaded);
    return same_file(loaded, argv[1]) ? 0 : 14;
}
"#,
    )
    .unwrap();
    build
        .executable(&SharedLibrary {
            name: "windows-core-loader-probe".into(),
            sources: vec![source],
            include_dirs: vec![
                root.into(),
                root.join("libobs"),
                root.join("deps/w32-pthreads"),
                build.output.join("config"),
                options
                    .native_dependencies
                    .as_ref()
                    .unwrap()
                    .join("include"),
            ],
            dependencies: vec![core.into()],
            compile_options: vec!["/utf-8".into()],
            c_compile_options: vec!["/std:c17".into()],
            link_options: vec!["kernel32.lib".into()],
            ..SharedLibrary::default()
        })
        .unwrap()
}

fn exports(library: &Path) -> BTreeSet<String> {
    let tool =
        std::env::var_os("OBS_BUILD_WINDOWS_EXPORT_TOOL").unwrap_or_else(|| "llvm-readobj".into());
    let dumpbin = Path::new(&tool)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .eq_ignore_ascii_case("dumpbin");
    let result = std::process::Command::new(tool)
        .arg(if dumpbin {
            "/exports"
        } else {
            "--coff-exports"
        })
        .arg(library)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let exports: BTreeSet<_> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| {
            if dumpbin {
                let fields: Vec<_> = line.split_whitespace().collect();
                (fields.len() >= 4
                    && fields[0].parse::<u32>().is_ok()
                    && u32::from_str_radix(fields[1], 16).is_ok()
                    && u32::from_str_radix(fields[2], 16).is_ok())
                .then(|| fields[3].to_owned())
            } else {
                line.trim().strip_prefix("Name: ").map(str::to_owned)
            }
        })
        .collect();
    assert!(
        !exports.is_empty(),
        "no exports parsed from {}",
        library.display()
    );
    exports
}

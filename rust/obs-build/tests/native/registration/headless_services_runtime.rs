use super::*;
use obs_build::validation::graph::Dependency;
use std::process::Command;

fn checked(command: &mut Command) -> std::process::Output {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{command:?}: {error}"));
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn assert_cases(output: &std::process::Output, name: &str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // Native diagnostics are separate from cmocka case records.
    // Reject cmocka skip records; the exact passed count also forbids hidden cases.
    assert!(
        !text.lines().any(|line| line.trim_start().starts_with('[')
            && line.to_ascii_lowercase().contains("skipped")),
        "{text}"
    );
    let count = match name {
        "test_inhibit_sleep" => 8,
        "test_file_updater_failure"
        | "test_frontend_warn_fallback"
        | "test_text_freetype2_face_index" => 2,
        "test_file_updater_timeout" => 1,
        "test_text_freetype2_bundled_font" | "test_text_freetype2_bundled_font_host_font" => 3,
        _ => panic!("unknown fixture"),
    };
    assert_eq!(
        text.matches(&format!("[  PASSED  ] {count} test(s)."))
            .count(),
        1,
        "{text}"
    );
    eprintln!("{name}: {text}");
}
#[test]
fn canonical_headless_services_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = serde_json::json!({"schema":1,"targets":{},"tests":{}});
    for name in CASES {
        selected["targets"][name] = raw["targets"][name].clone();
    }
    for (name, spec) in raw["tests"].as_object().unwrap() {
        if CASES.contains(&spec["target"].as_str().unwrap()) {
            selected["tests"][name] = spec.clone();
        }
    }
    let graph = Graph::parse(&selected.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let core = built_core(&build, &root);
    let api = obs_build::frontend::build_api(&build, &root, &core.dependency.artifacts[0]).unwrap();
    let mut interfaces: std::collections::BTreeMap<String, Dependency> = [
        ("OBS::libobs".into(), core.dependency),
        (
            "OBS::frontend-api".into(),
            Dependency {
                artifacts: vec![api],
                include_dirs: vec![root.join("frontend/api")],
                ..Default::default()
            },
        ),
    ]
    .into();
    for (name, package) in [
        ("OBS::native-test-cmocka", "cmocka"),
        ("CURL::libcurl", "libcurl"),
        ("Freetype::Freetype", "freetype2"),
        ("Fontconfig::Fontconfig", "fontconfig"),
    ] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    prepare_fonts(&build.output, &root);
    let facts = [
        ("OS_LINUX".into(), true),
        ("OS_FREEBSD".into(), false),
        ("HAVE_FONTCONFIG_TEST_OWNER".into(), true),
        ("HAVE_FRONTEND_WARN_TEST_OWNER".into(), true),
    ]
    .into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), 6);
    for (name, test) in &graph.tests {
        assert_cases(
            &checked(
                test.command(&root, &build.output, &artifacts[&test.target])
                    .unwrap()
                    .current_dir(directory.path()),
            ),
            name,
        );
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(directory.path(), &root, &core.cmake);
    replay_cmake(directory.path());
}

// Mirror the original CMake fixtures: only the copied collection or bundled
// fonts are visible, with a separate host-font configuration for the second mode.
fn prepare_fonts(output: &Path, root: &Path) {
    let collection = output.join("ft2-collection-fonts");
    fs::create_dir_all(&collection).unwrap();
    fs::copy(
        root.join("test/cmocka/data/text-collection.ttc"),
        collection.join("text-collection.ttc"),
    )
    .unwrap();
    let fonts = root.join("plugins/text-freetype2/data/fonts");
    for (name, directory) in [
        ("collection", Some(collection.as_path())),
        ("isolated", None),
        ("host", Some(fonts.as_path())),
    ] {
        let dir = directory.map_or_else(String::new, |p| format!("<dir>{}</dir>", p.display()));
        fs::write(output.join(format!("ft2-{name}-fonts.conf")), format!("<?xml version=\"1.0\"?>\n<fontconfig>{dir}<cachedir>{}/ft2-{name}-fonts-cache</cachedir></fontconfig>\n", output.display())).unwrap();
    }
}

fn write_cmake(directory: &Path, root: &Path, core: &str) {
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(directory.join("CMakeLists.txt"), format!(r#"cmake_minimum_required(VERSION 3.28)
project(headless-services-contract C CXX)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Linux selection" ON)
option(OS_FREEBSD "FreeBSD selection" OFF)
option(OS_WINDOWS "Windows selection" OFF)
option(OS_MACOS "macOS selection" OFF)
option(ENABLE_FRONTEND "Frontend API owner" ON)
option(WITH_FONTS "Text plugin owner" ON)
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
find_package(Freetype REQUIRED)
find_package(Fontconfig REQUIRED)
find_package(CURL REQUIRED)
if(WITH_FONTS)
 add_library(OBS::text-freetype2 INTERFACE IMPORTED)
endif()
add_library(frontend-api SHARED "{source}/frontend/api/build/obs-frontend-api.core+.hpp.cpp")
target_include_directories(frontend-api PRIVATE "{source}")
target_include_directories(frontend-api PUBLIC "{source}/frontend/api")
target_link_libraries(frontend-api PRIVATE OBS::libobs)
add_library(OBS::frontend-api ALIAS frontend-api)
foreach(name IN ITEMS test_inhibit_sleep test_file_updater_failure test_file_updater_timeout test_frontend_warn_fallback test_text_freetype2_face_index test_text_freetype2_bundled_font)
 include("{source}/test/cmocka/tests.d/${{name}}.cmake")
endforeach()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#)).unwrap();
}
fn replay_cmake(directory: &Path) {
    for (mode, options, count) in [
        ("enabled", vec![], 7),
        ("windows", vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"], 0),
        ("macos", vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"], 0),
        ("no-fonts", vec!["-DWITH_FONTS=OFF"], 4),
        ("no-frontend", vec!["-DENABLE_FRONTEND=OFF"], 6),
        ("freebsd", vec!["-DOS_LINUX=OFF", "-DOS_FREEBSD=ON"], 1),
    ] {
        let output = directory.join(mode);
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(directory)
                .arg("-B")
                .arg(&output)
                .arg("-DCMAKE_BUILD_TYPE=RelWithDebInfo")
                .args(options),
        );
        if mode == "enabled" {
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--parallel", "2"]),
            );
            for name in CASES
                .into_iter()
                .chain(["test_text_freetype2_bundled_font_host_font"])
            {
                let result = checked(
                    Command::new("ctest")
                        .arg("--test-dir")
                        .arg(&output)
                        .args([
                            "--output-on-failure",
                            "--no-tests=error",
                            "-V",
                            "-R",
                            &format!("^{name}$"),
                        ])
                        .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
                );
                assert_cases(&result, name);
            }
        }
        check_inventory(&output, count, mode == "enabled");
    }
}

fn check_inventory(output: &Path, count: usize, built: bool) {
    let result = checked(
        Command::new("ctest")
            .arg("--test-dir")
            .arg(output)
            .arg("--show-only=json-v1"),
    );
    let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    assert_eq!(tests.len(), count, "{}", output.display());
    for test in tests {
        let name = test["name"].as_str().unwrap();
        assert!(CASES.contains(&name) || name == "test_text_freetype2_bundled_font_host_font");
        let properties = test["properties"].as_array().unwrap();
        assert!(!properties.iter().any(|p| p["name"] == "SKIP_RETURN_CODE"));
        if built {
            let command = test["command"].as_array().unwrap();
            assert_eq!(command.len(), 1);
            assert_eq!(
                Path::new(command[0].as_str().unwrap()).file_name().unwrap(),
                test["name"]
                    .as_str()
                    .unwrap()
                    .strip_suffix("_host_font")
                    .unwrap_or(name)
            );
        }
    }
}

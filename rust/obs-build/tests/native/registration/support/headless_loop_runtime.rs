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
    // Frame-pacing diagnostics describe intentionally skipped video frames.
    // Reject cmocka skip records; the exact passed count also forbids hidden cases.
    assert!(
        !text.lines().any(|line| line.trim_start().starts_with('[')
            && line.to_ascii_lowercase().contains("skipped")),
        "{text}"
    );
    let count = match name {
        "test_python_stdio" | "test_pipewire_connect_start" | "test_pipewire_teardown_wait" => 2,
        "test_pipewire_frame_pacing" => 7,
        "test_xshm_capture_pipeline" => 3,
        "test_mac_vmic_shm" => 4,
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
fn canonical_headless_loop_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let mut selected = serde_json::json!({"schema":1,"targets":{},"tests":{}});
    for section in ["targets", "tests"] {
        for name in CASES {
            selected[section][name] = raw[section][name].clone();
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
    let mut interfaces: std::collections::BTreeMap<String, Dependency> =
        [("OBS::libobs".into(), core.dependency)].into();
    for (name, package) in [
        ("OBS::native-test-cmocka", "cmocka"),
        ("Python::Python", "python3-embed"),
        ("PipeWire::PipeWire", "libpipewire-0.3"),
        ("Libdrm::Libdrm", "libdrm"),
        ("gio::gio", "gio-2.0"),
        ("OBS::native-test-glib", "gobject-2.0"),
        ("X11::X11", "x11"),
        ("X11::XCB", "x11-xcb"),
        ("XCB::XCB", "xcb"),
        ("XCB::XFIXES", "xcb-xfixes"),
        ("XCB::RANDR", "xcb-randr"),
        ("XCB::SHM", "xcb-shm"),
        ("XCB::XINERAMA", "xcb-xinerama"),
        ("XCB::COMPOSITE", "xcb-composite"),
    ] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    let glad = SharedLibrary {
        name: "native-headless-glad".into(),
        sources: vec![root.join("deps/glad/src/glad.c")],
        include_dirs: vec![root.join("deps/glad/include")],
        ..Default::default()
    };
    interfaces.insert(
        "OBS::glad".into(),
        Dependency {
            artifacts: vec![build.static_library(&glad).unwrap()],
            include_dirs: glad.include_dirs,
            ..Default::default()
        },
    );
    let facts = [
        ("OS_LINUX".into(), true),
        ("OS_FREEBSD".into(), false),
        ("OS_OPENBSD".into(), false),
        ("MSVC".into(), false),
        ("HAVE_LINUX_CAPTURE".into(), true),
        ("HAVE_PIPEWIRE_HEADLESS_SDK".into(), true),
        ("HAVE_PIPEWIRE_LOOP_TEST_SDK".into(), true),
        ("HAVE_PYTHON_EMBED_TEST_SDK".into(), true),
    ]
    .into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), 6);
    for name in CASES {
        assert_cases(
            &checked(
                &mut graph.tests[name]
                    .command(&root, &build.output, &artifacts[name])
                    .unwrap(),
            ),
            name,
        );
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(directory.path(), &root, &core.cmake);
    replay_cmake(directory.path());
}
fn write_cmake(directory: &Path, root: &Path, core: &str) {
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(directory.join("CMakeLists.txt"),format!(r#"cmake_minimum_required(VERSION 3.28)
project(headless-loop-contract C)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Linux selection" ON)
option(OS_FREEBSD "FreeBSD selection" OFF)
option(OS_OPENBSD "OpenBSD selection" OFF)
option(WITH_OWNERS "Owning plugins" ON)
option(WITH_PYTHON "Python SDK" ON)
option(WITH_PIPEWIRE "PipeWire SDK" ON)
option(WITH_LOOP_SDK "Versioned loop SDK" ON)
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
list(APPEND CMAKE_MODULE_PATH "{source}/cmake/finders")
find_package(ECM REQUIRED NO_MODULE)
list(APPEND CMAKE_MODULE_PATH "${{ECM_FIND_MODULE_DIR}}")
add_library(native-glad STATIC "{source}/deps/glad/src/glad.c")
target_include_directories(native-glad PUBLIC "{source}/deps/glad/include")
add_library(OBS::glad ALIAS native-glad)
if(WITH_OWNERS)
 add_library(OBS::pipewire INTERFACE IMPORTED)
 add_library(linux-capture INTERFACE IMPORTED)
endif()
if(NOT WITH_PIPEWIRE)
 set(CMAKE_DISABLE_FIND_PACKAGE_PipeWire TRUE)
endif()
if(NOT WITH_PYTHON)
 set(CMAKE_DISABLE_FIND_PACKAGE_Python TRUE)
endif()
foreach(name IN ITEMS test_python_stdio test_pipewire_frame_pacing test_pipewire_connect_start test_pipewire_teardown_wait test_xshm_capture_pipeline test_mac_vmic_shm)
 include("{source}/test/cmocka/tests.d/${{name}}.cmake")
endforeach()
if(NOT WITH_LOOP_SDK)
 set(HAVE_PIPEWIRE_LOOP_TEST_SDK FALSE)
endif()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#)).unwrap();
}
fn replay_cmake(directory: &Path) {
    for (mode, options, count) in [
        ("enabled", vec![], 6),
        (
            "windows",
            vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON", "-DWITH_OWNERS=OFF"],
            1,
        ),
        (
            "macos",
            vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON", "-DWITH_OWNERS=OFF"],
            1,
        ),
        ("no-owner", vec!["-DWITH_OWNERS=OFF"], 2),
        ("no-python", vec!["-DWITH_PYTHON=OFF"], 5),
        ("no-pipewire", vec!["-DWITH_PIPEWIRE=OFF"], 3),
        ("old-loop-sdk", vec!["-DWITH_LOOP_SDK=OFF"], 5),
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
            for name in CASES {
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
        assert!(CASES.contains(&name));
        let properties = test["properties"].as_array().unwrap();
        assert!(!properties.iter().any(|p| p["name"] == "SKIP_RETURN_CODE"));
        if built {
            let command = test["command"].as_array().unwrap();
            assert_eq!(command.len(), 1);
            assert_eq!(
                Path::new(command[0].as_str().unwrap()).file_name().unwrap(),
                name
            );
        }
    }
}

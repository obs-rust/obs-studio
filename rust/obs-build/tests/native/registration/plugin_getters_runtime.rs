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
    assert!(!text.to_ascii_lowercase().contains("skipped"), "{text}");
    if name == "test_obs_outputs_rtmp_host" {
        assert_eq!(text.matches("[  PASSED  ] 12 test(s).").count(), 1, "{text}");
    }
    eprintln!("{text}");
}

/// Refs #1390: both consumers execute every headless ABI case headlessly.
#[test]
fn canonical_plugin_getters_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
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
    let mut interfaces: std::collections::BTreeMap<String, Dependency> = [
        ("OBS::libobs".into(), core.dependency),
        (
            "OBS::caption".into(),
            Dependency {
                include_dirs: vec![root.join("deps/libcaption")],
                ..Default::default()
            },
        ),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    add_x11_interfaces(&build, &root, &mut interfaces);
    interfaces.insert("MbedTLS::mbedtls".into(), Dependency {
        libraries: vec!["mbedtls".into(), "mbedx509".into(), "mbedcrypto".into()],
        ..Default::default()
    });
    let facts = [
        ("OS_WINDOWS".into(), false),
        ("OS_LINUX".into(), true),
        ("HAVE_PLUGIN_GETTER_OWNERS".into(), true),
        ("HAVE_MBEDTLS_TEST_SDK".into(), true),
    ].into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    // Require every unchanged cmocka case without skips.
    for name in CASES {
        eprintln!("Cargo {name}");
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
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"
cmake_minimum_required(VERSION 3.28)
project(plugin-getters-contract C)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Evaluate Linux selection" ON)
option(GETTER_OWNERS "Evaluate owning plugins" ON)
option(OS_WINDOWS "Evaluate original Windows exclusion" OFF)
option(OS_MACOS "Evaluate original macOS selection" OFF)
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
add_library(OBS::caption INTERFACE IMPORTED)
set_target_properties(OBS::caption PROPERTIES INTERFACE_INCLUDE_DIRECTORIES "{source}/deps/libcaption")
find_package(ECM REQUIRED NO_MODULE)
list(APPEND CMAKE_MODULE_PATH "${{ECM_FIND_MODULE_DIR}}" "{source}/cmake/finders")
add_library(native-glad STATIC "{source}/deps/glad/src/glad.c")
target_include_directories(native-glad PUBLIC "{source}/deps/glad/include")
add_library(OBS::glad ALIAS native-glad)
find_package(X11 REQUIRED)
find_package(X11_XCB REQUIRED)
find_package(XCB REQUIRED COMPONENTS XCB XFIXES RANDR SHM XINERAMA COMPOSITE)
find_package(MbedTLS REQUIRED)
if(GETTER_OWNERS)
  foreach(owner IN ITEMS obs-filters image-source obs-transitions linux-capture)
    add_library(${{owner}} INTERFACE)
  endforeach()
endif()
include("{source}/test/cmocka/tests.d/test_plugin_getter_races.cmake")
include("{source}/test/cmocka/tests.d/test_obs_outputs_rtmp_host.cmake")
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#
        ),
    )
    .unwrap();
}

fn replay_cmake(directory: &Path) {
    for (mode, options, count) in [
        ("enabled", vec![], CASES.len()),
        (
            "windows",
            vec!["-DOS_WINDOWS=ON", "-DOS_LINUX=OFF", "-DGETTER_OWNERS=OFF"],
            0,
        ),
        (
            "macos",
            vec!["-DOS_MACOS=ON", "-DOS_LINUX=OFF", "-DGETTER_OWNERS=OFF"],
            1,
        ),
        ("no-owner", vec!["-DGETTER_OWNERS=OFF"], 1),
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

fn add_x11_interfaces(
    build: &NativeBuild,
    root: &Path,
    interfaces: &mut std::collections::BTreeMap<String, Dependency>,
) {
    for (name, package) in [
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
        name: "native-fixture-glad".into(),
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
}

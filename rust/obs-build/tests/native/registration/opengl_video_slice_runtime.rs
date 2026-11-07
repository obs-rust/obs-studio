use super::*;
use obs_build::validation::graph::{Dependency, Graph};
const CASES: [&str; 2] = [
    "test_visibility_transition",
    "test_xcomposite_error_replies",
];
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

fn assert_no_skips(output: &std::process::Output) {
    assert!(
        ![output.stdout.as_slice(), output.stderr.as_slice()]
            .iter()
            .any(|stream| String::from_utf8_lossy(stream)
                .to_ascii_lowercase()
                .contains("skipped")),
        "skipped native cases"
    );
}

#[test]
fn canonical_opengl_video_slice_runs_unchanged() {
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
    let mut core = Dependency::package(&build.target, "libobs").unwrap();
    let nested: Vec<_> = core
        .include_dirs
        .iter()
        .map(|path| path.join("obs"))
        .filter(|path| path.is_dir())
        .collect();
    core.include_dirs.extend(nested);
    core.include_dirs.insert(0, root.join("libobs"));
    core.compile_options.push("-fvisibility=hidden".into());
    let mut interfaces: std::collections::BTreeMap<String, Dependency> = [
        ("OBS::libobs".into(), core),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    let libdir = checked(Command::new("pkg-config").args(["--variable=libdir", "libobs"]));
    let renderer =
        Path::new(std::str::from_utf8(&libdir.stdout).unwrap().trim()).join("libobs-opengl.so");
    assert!(renderer.is_file());
    interfaces.insert(
        "OBS::libobs-opengl".into(),
        Dependency {
            artifacts: vec![renderer],
            ..Default::default()
        },
    );
    interfaces.insert(
        "m".into(),
        Dependency {
            link_options: vec!["-lm".into()],
            ..Default::default()
        },
    );
    add_x11_interfaces(&build, &root, &mut interfaces);
    let facts = [
        ("OS_LINUX".into(), true),
        ("HAVE_OBS_OPENGL".into(), true),
        ("HAVE_LINUX_CAPTURE".into(), true),
    ]
    .into();
    let runtime = directory.path().join("runtime");
    fs::create_dir_all(runtime.join("data")).unwrap();
    checked(
        Command::new("cp")
            .args(["-R"])
            .arg(root.join("libobs/data"))
            .arg(runtime.join("data/libobs")),
    );
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for name in CASES {
        let result = checked(
            graph.tests[name]
                .command_in(&root, &build.output, &artifacts[name], &runtime)
                .unwrap()
                .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
        );
        assert_no_skips(&result);
        eprintln!("Cargo {name}: {}", String::from_utf8_lossy(&result.stdout));
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(directory.path(), &root, &runtime);
    replay_cmake(directory.path());
}

fn write_cmake(directory: &Path, root: &Path, runtime: &Path) {
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(directory.join("CMakeLists.txt"), format!(r#"cmake_minimum_required(VERSION 3.28)
project(opengl-video-contract C)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Original Linux guard" ON)
option(HAVE_OBS_OPENGL "Original renderer guard" ON)
option(CAPTURE_OWNER "Original owning plugin guard" ON)
find_package(ECM REQUIRED NO_MODULE)
list(APPEND CMAKE_MODULE_PATH "${{ECM_FIND_MODULE_DIR}}")
add_library(native-glad STATIC "{source}/deps/glad/src/glad.c")
target_include_directories(native-glad PUBLIC "{source}/deps/glad/include")
add_library(OBS::glad ALIAS native-glad)
if(CAPTURE_OWNER)
  add_library(linux-capture INTERFACE)
endif()
set(OBS_OUTPUT_DIR "{runtime}")
find_package(PkgConfig REQUIRED)
pkg_check_modules(CORE REQUIRED IMPORTED_TARGET libobs)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
foreach(dir IN LISTS CORE_INCLUDE_DIRS)
  if(IS_DIRECTORY "${{dir}}/obs")
    target_include_directories(PkgConfig::CORE INTERFACE "${{dir}}/obs")
  endif()
endforeach()
target_include_directories(PkgConfig::CORE BEFORE INTERFACE "{source}/libobs")
add_library(OBS::libobs ALIAS PkgConfig::CORE)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
if(HAVE_OBS_OPENGL)
  add_library(OBS::libobs-opengl MODULE IMPORTED)
  set_property(TARGET OBS::libobs-opengl PROPERTY IMPORTED_LOCATION "${{CORE_LIBDIR}}/libobs-opengl.so")
endif()
include("{source}/test/cmocka/tests.d/test_xcomposite_error_replies.cmake")
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#, runtime=runtime.display())).unwrap();
}

fn replay_cmake(directory: &Path) {
    for (mode, options, expected) in [
        ("enabled", vec![], CASES.to_vec()),
        ("windows", vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"], vec![]),
        ("macos", vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"], vec![]),
        (
            "no-owner",
            vec!["-DCAPTURE_OWNER=OFF"],
            vec!["test_visibility_transition"],
        ),
        ("no-renderer", vec!["-DHAVE_OBS_OPENGL=OFF"], vec![]),
    ] {
        let output = directory.join(mode);
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(directory)
                .arg("-B")
                .arg(&output)
                .args(options),
        );
        if mode == "enabled" {
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--parallel", "2"]),
            );
            let result = checked(
                Command::new("ctest")
                    .arg("--test-dir")
                    .arg(&output)
                    .args(["--output-on-failure", "--no-tests=error", "-V"])
                    .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
            );
            assert_no_skips(&result);
            eprintln!("CMake: {}", String::from_utf8_lossy(&result.stdout));
        }
        check_inventory(&output, &expected, mode == "enabled");
    }
}

fn check_inventory(output: &Path, expected: &[&str], built: bool) {
    let result = checked(
        Command::new("ctest")
            .arg("--test-dir")
            .arg(output)
            .arg("--show-only=json-v1"),
    );
    let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    assert_eq!(tests.len(), expected.len(), "{}", output.display());
    for test in tests {
        let name = test["name"].as_str().unwrap();
        assert!(expected.contains(&name));
        if built {
            assert_eq!(test["command"].as_array().unwrap().len(), 1);
            assert_eq!(
                Path::new(test["command"][0].as_str().unwrap())
                    .file_name()
                    .unwrap(),
                name
            );
        }
        let timeout = test["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|property| property["name"] == "TIMEOUT");
        let expected = match name {
            "test_visibility_transition" => None,
            _ => Some(60.0),
        };
        assert_eq!(timeout.map(|p| p["value"].as_f64().unwrap()), expected);
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

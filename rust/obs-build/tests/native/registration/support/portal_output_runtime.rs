use super::*;
use obs_build::validation::graph::Dependency;
use std::process::{Command, Output};

fn checked(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn assert_cases(output: &Output, name: &str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .replace("\r\n", "\n");
    let count = match name {
        "test_encoder_counter_getters" | "test_config_close_fault" => 2,
        "test_inhibit_portal" => 9,
        "test_inhibit_sleep_dbus" => 10,
        "test_portal_shortcuts" => 32,
        "test_output_stop_while_reconnecting" => 3,
        _ => panic!("unknown fixture {name}"),
    };
    assert!(
        text.contains(&format!("[  PASSED  ] {count} test(s).")),
        "{name}: {text}"
    );
    assert!(!text.contains("[  SKIPPED ]"), "{name}: {text}");
    assert!(!text.contains("[  FAILED  ]"), "{name}: {text}");
}

fn binding(binary: &Path, core: &Path) {
    let output = checked(Command::new("ldd").arg(binary));
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("libobs.so"))
        .expect("selected core binding");
    let loaded = line
        .split_once("=>")
        .unwrap()
        .1
        .split_whitespace()
        .next()
        .unwrap();
    assert_eq!(
        dunce::canonicalize(loaded).unwrap(),
        dunce::canonicalize(core).unwrap(),
        "{text}"
    );
    eprintln!("Selected core {}: {}", binary.display(), core.display());
}

#[test]
fn canonical_portal_output_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let core = built_core(&build, &root);
    let core_path = core.dependency.artifacts[0].clone();
    let caption = obs_build::libobs::build_caption(&build, &root).unwrap();
    let renderer =
        obs_build::graphics::build_opengl(&build, &root, &core_path, &Default::default()).unwrap();
    let runtime = directory.path().join("runtime");
    for data in ["share/obs", "RelWithDebInfo/share/obs"] {
        fs::create_dir_all(runtime.join(data)).unwrap();
        checked(
            Command::new("cp")
                .arg("-R")
                .arg(root.join("libobs/data"))
                .arg(runtime.join(data).join("libobs")),
        );
    }
    let shim = obs_build::validation::build_config_close_fault(&build, &root).unwrap();
    let interfaces = interfaces(&build, &root, core.dependency, &caption, &renderer, shim);
    let facts = std::collections::BTreeMap::from([
        ("UNIX".into(), true),
        ("OS_LINUX".into(), true),
        ("OS_FREEBSD".into(), false),
        ("OS_WINDOWS".into(), false),
        ("OS_MACOS".into(), false),
        ("HAVE_PORTAL_TEST_GIO".into(), true),
        ("HAVE_OBS_OPENGL".into(), true),
    ]);
    let mut graph = expected();
    for target in graph.targets.values_mut() {
        target.link_options.extend([
            "-Wl,--as-needed".into(),
            "-Wl,--no-copy-dt-needed-entries".into(),
        ]);
    }
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), 6);
    let raw: serde_json::Value =
        serde_json::from_str(include_str!("../portal_output_expected.json")).unwrap();
    fs::write(directory.path().join("graph.json"), raw.to_string()).unwrap();
    write_cmake(
        directory.path(),
        &root,
        &core.cmake,
        &caption,
        &renderer,
        &runtime,
    );
    let output = directory.path().join("cmake-output");
    checked(
        Command::new("cmake")
            .arg("-S")
            .arg(directory.path())
            .arg("-B")
            .arg(&output)
            .args(["-G", "Ninja", "-DCMAKE_BUILD_TYPE=RelWithDebInfo"]),
    );
    checked(
        Command::new("cmake")
            .arg("--build")
            .arg(&output)
            .args(["--parallel", "4"]),
    );
    for name in graph.tests.keys() {
        binding(&output.join(name), &core_path);
        let result = checked(
            Command::new("ctest")
                .arg("--test-dir")
                .arg(&output)
                .args(["-V", "--no-tests=error", "--repeat", "until-fail:5", "-R"])
                .arg(format!("^{name}$"))
                .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
        );
        assert_cases(&result, name);
        eprintln!("CMake {name}: {}", String::from_utf8_lossy(&result.stdout));
    }
    for (name, artifact) in artifacts {
        binding(&artifact, &core_path);
        for iteration in 1..=5 {
            let output = checked(
                graph.tests[&name]
                    .command_in(&root, &build.output, &artifact, &runtime)
                    .unwrap()
                    .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
            );
            assert_cases(&output, &name);
            eprintln!(
                "Cargo {name} iteration {iteration}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

fn interfaces(
    build: &NativeBuild,
    root: &Path,
    core: Dependency,
    caption: &Path,
    renderer: &Path,
    shim: std::path::PathBuf,
) -> std::collections::BTreeMap<String, Dependency> {
    let mut interfaces = std::collections::BTreeMap::from([
        ("OBS::libobs".into(), core),
        (
            "OBS::caption".into(),
            Dependency {
                artifacts: vec![caption.to_owned()],
                include_dirs: vec![root.join("deps/libcaption")],
                ..Default::default()
            },
        ),
        ("Uthash::Uthash".into(), Dependency::default()),
        (
            "OBS::libobs-opengl".into(),
            Dependency {
                artifacts: vec![renderer.to_owned()],
                ..Default::default()
            },
        ),
        (
            "config_close_fault".into(),
            Dependency {
                artifacts: vec![shim],
                ..Default::default()
            },
        ),
    ]);
    for (name, package) in [
        ("OBS::native-test-cmocka", "cmocka"),
        ("gio::gio", "gio-2.0"),
        ("PkgConfig::PORTAL_TEST_GLIB", "glib-2.0"),
        ("PkgConfig::INHIBIT_TEST_GLIB", "glib-2.0"),
    ] {
        let mut dependency = Dependency::package(&build.target, package).unwrap();
        if package == "glib-2.0" {
            let object = Dependency::package(&build.target, "gobject-2.0").unwrap();
            dependency.link_options.extend(object.link_options);
            dependency.include_dirs.extend(object.include_dirs);
        }
        interfaces.insert(name.into(), dependency);
    }
    interfaces
}

fn write_cmake(
    directory: &Path,
    root: &Path,
    core: &str,
    caption: &Path,
    renderer: &Path,
    runtime: &Path,
) {
    let cmake = format!(
        r#"cmake_minimum_required(VERSION 3.28)
project(portal-output-contract C)
enable_testing()
set(OS_LINUX ON)
set(OS_FREEBSD OFF)
set(OS_WINDOWS OFF)
set(OS_MACOS OFF)
set(OBS_OUTPUT_DIR "{runtime}")
set(CMAKE_MODULE_PATH "{root}/cmake/finders")
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
find_package(Uthash REQUIRED)
add_library(OBS::caption STATIC IMPORTED)
set_target_properties(OBS::caption PROPERTIES IMPORTED_LOCATION "{caption}" INTERFACE_INCLUDE_DIRECTORIES "{root}/deps/libcaption")
add_library(OBS::libobs-opengl MODULE IMPORTED)
set_target_properties(OBS::libobs-opengl PROPERTIES IMPORTED_LOCATION "{renderer}")
set(HAVE_OBS_OPENGL ON)
set(CMAKE_SOURCE_DIR "{root}")
include("{root}/test/cmocka/tests.d/test_config_close_fault.cmake")
include("{root}/test/cmocka/native-test-graph.cmake")
add_link_options("LINKER:--as-needed" "LINKER:--no-copy-dt-needed-entries")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{root}")
"#,
        root = root.to_string_lossy().replace('\\', "/"),
        core = core,
        runtime = runtime.to_string_lossy().replace('\\', "/"),
        caption = caption.to_string_lossy().replace('\\', "/"),
        renderer = renderer.to_string_lossy().replace('\\', "/")
    );
    fs::write(directory.join("CMakeLists.txt"), cmake).unwrap();
}

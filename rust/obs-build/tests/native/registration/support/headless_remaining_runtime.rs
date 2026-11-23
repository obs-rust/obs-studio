use super::*;
use obs_build::validation::graph::Dependency;
use std::process::Command;

fn checked(command: &mut Command) -> std::process::Output {
    let output = command
        .output()
        .unwrap_or_else(|e| panic!("{command:?}: {e}"));
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
    assert!(
        !text.lines().any(|line| line.trim_start().starts_with('[')
            && line.to_ascii_lowercase().contains("skipped")),
        "{text}"
    );
    let count = match name {
        "test_core_module_manifest" => 1,
        "test_preflight_abi" => 6,
        "test_scripting_obs_defs" => 2,
        _ => 0,
    };
    if count > 0 {
        assert_eq!(
            text.matches(&format!("[  PASSED  ] {count} test(s)."))
                .count(),
            1,
            "{text}"
        );
    }
    eprintln!("{name}: {text}");
}
#[test]
fn canonical_headless_remaining_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let mut selected = expected();
    for section in ["targets", "tests"] {
        for (name, value) in selected[section].as_object_mut().unwrap() {
            assert_eq!(*value, raw[section][name], "{section}/{name}");
            *value = raw[section][name].clone();
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
    let (interfaces, imports) = dependencies(&build, &root, &core.dependency);
    let facts = facts("OS_LINUX", true);
    // NativeProfile defaults to RelWithDebInfo, which defines NDEBUG.
    // These unchanged frontend fixtures use assert(), so compile test executables
    // in Debug while retaining the selected core's normal optimized build.
    let fixture_build = build.with_profile(obs_build::NativeProfile::Debug);
    let artifacts = graph
        .compile(&fixture_build, &root, &facts, &interfaces)
        .unwrap();
    assert_eq!(artifacts.len(), 6);
    for (name, test) in &graph.tests {
        assert_cases(
            &checked(
                &mut test
                    .command(&root, &build.output, &artifacts[&test.target])
                    .unwrap(),
            ),
            name,
        );
    }
    // The auxiliary configure regression keeps its original CTest owner and
    // generator/toolchain forwarding; replay it explicitly for Cargo as well.
    let auxiliary = checked(
        Command::new("cmake")
            .arg(format!(
                "-DOBS_CORE_MODULES_CMAKE={}",
                root.join("libobs/cmake/obs-core-modules.cmake").display()
            ))
            .arg(format!(
                "-DTEST_FIXTURE_ROOT={}",
                directory.path().join("cargo-manifest-source").display()
            ))
            .arg(format!(
                "-DTEST_C_COMPILER={}",
                std::env::var("CC").unwrap_or_else(|_| "cc".into())
            ))
            .arg("-DTEST_BUILD_CONFIG=Debug")
            .arg("-P")
            .arg(root.join("test/cmocka/check_core_module_manifest_source.cmake")),
    );
    assert_cases(&auxiliary, "test_core_module_manifest_source");
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(
        directory.path(),
        &root,
        &format!("{}{}", core.cmake, imports),
    );
    replay_cmake(directory.path());
}
fn facts(os: &str, enabled: bool) -> std::collections::BTreeMap<String, bool> {
    let mut facts: std::collections::BTreeMap<String, bool> = [
        "OS_LINUX",
        "OS_FREEBSD",
        "OS_OPENBSD",
        "OS_WINDOWS",
        "OS_MACOS",
    ]
    .into_iter()
    .map(|n| (n.into(), n == os))
    .collect();
    for n in [
        "HAVE_FRONTEND_PLAIN_TEST_OWNER",
        "HAVE_X264_TEST_OWNER",
        "ENABLE_CORE_MODULES",
        "HAVE_PREFLIGHT_TEST_ARCHIVE",
        "HAVE_SCRIPTING_DEFS_TEST_SDK",
    ] {
        facts.insert(n.into(), enabled);
    }
    facts.insert(
        "C_COMPILER_CLANG".into(),
        std::env::var("CC").unwrap_or_default().contains("clang"),
    );
    facts
}
fn dependencies(
    build: &NativeBuild,
    root: &Path,
    core: &Dependency,
) -> (std::collections::BTreeMap<String, Dependency>, String) {
    let archive = std::env::var_os("OBS_PREFLIGHT_TEST_ARCHIVE")
        .expect("build obs-preflight-ffi through soldr first");
    let manifest = obs_build::validation::sdk_helpers::core_manifest(
        build,
        root,
        &["obs-outputs", "obs-transitions"],
    )
    .unwrap();
    let scripting = obs_build::validation::sdk_helpers::scripting_defs(build, root, core).unwrap();
    let parser = build
        .static_library(&SharedLibrary {
            name: "headless-opts-parser".into(),
            sources: vec![root.join("shared/opts-parser/build/opts-parser.core+.h.c")],
            include_dirs: vec![
                root.into(),
                root.join("libobs"),
                root.join("shared/opts-parser"),
            ],
            ..Default::default()
        })
        .unwrap();
    let mut parser = Dependency {
        artifacts: vec![parser],
        include_dirs: vec![root.join("shared/opts-parser")],
        ..Default::default()
    };
    parser.artifacts.extend(core.artifacts.iter().cloned());
    parser
        .include_dirs
        .extend(core.include_dirs.iter().cloned());
    let interfaces = [
        ("OBS::libobs".into(), core.clone()),
        ("OBS::opts-parser".into(), parser),
        ("OBS::libobs-core-modules".into(), manifest),
        ("OBS::scripting-defs-test".into(), scripting),
        (
            "OBS::preflight-test-archive".into(),
            Dependency {
                artifacts: vec![archive.into()],
                ..Default::default()
            },
        ),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
        (
            "Threads::Threads".into(),
            Dependency {
                link_options: vec!["-pthread".into()],
                ..Default::default()
            },
        ),
        (
            "dl".into(),
            Dependency {
                link_options: vec!["-ldl".into()],
                ..Default::default()
            },
        ),
    ]
    .into();
    let imports = imports(&interfaces);
    (interfaces, imports)
}
fn imports(interfaces: &std::collections::BTreeMap<String, Dependency>) -> String {
    let mut result = String::new();
    for (name, dep) in interfaces {
        if name == "OBS::libobs" {
            continue;
        }
        result.push_str(&format!("add_library({name} INTERFACE IMPORTED)\n"));
        let includes = dep
            .include_dirs
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(";");
        let mut links = dep
            .artifacts
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        links.extend(dep.link_options.iter().cloned());
        let definitions = dep
            .definitions
            .iter()
            .map(|(key, value)| match value {
                Some(value) => format!("{key}={value}"),
                None => key.clone(),
            })
            .collect::<Vec<_>>()
            .join(";")
            .replace('"', "\\\"");
        result.push_str(&format!("set_target_properties({name} PROPERTIES INTERFACE_INCLUDE_DIRECTORIES \"{includes}\" INTERFACE_LINK_LIBRARIES \"{}\" INTERFACE_COMPILE_DEFINITIONS \"{definitions}\")\n", links.join(";")));
    }
    result
}
fn write_cmake(directory: &Path, root: &Path, core: &str) {
    fs::copy(
        root.join("test/cmocka/check_core_module_manifest_source.cmake"),
        directory.join("check_core_module_manifest_source.cmake"),
    )
    .unwrap();
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(headless-remaining-contract C CXX)
enable_testing()
{core}
option(OS_LINUX "Linux selection" ON)
option(OS_FREEBSD "FreeBSD selection" OFF)
option(OS_OPENBSD "OpenBSD selection" OFF)
option(OS_WINDOWS "Windows selection" OFF)
option(OS_MACOS "macOS selection" OFF)
option(HAVE_FRONTEND_PLAIN_TEST_OWNER "Frontend owner" ON)
option(HAVE_X264_TEST_OWNER "x264 owner" ON)
option(ENABLE_CORE_MODULES "Core modules owner" ON)
option(HAVE_PREFLIGHT_TEST_ARCHIVE "Preflight archive" ON)
option(HAVE_SCRIPTING_DEFS_TEST_SDK "Scripting SDKs" ON)
set(CMAKE_SOURCE_DIR "{source}")
include("{source}/test/cmocka/tests.d/test_core_module_manifest.cmake")
set(C_COMPILER_CLANG FALSE)
if(CMAKE_C_COMPILER_ID MATCHES "Clang")
 set(C_COMPILER_CLANG TRUE)
endif()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#
        ),
    )
    .unwrap();
}
fn replay_cmake(directory: &Path) {
    for (mode, options, count) in [
        ("enabled", vec![], 12),
        ("windows", vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"], 10),
        ("macos", vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"], 11),
        (
            "no-owners",
            vec![
                "-DHAVE_FRONTEND_PLAIN_TEST_OWNER=OFF",
                "-DHAVE_X264_TEST_OWNER=OFF",
                "-DENABLE_CORE_MODULES=OFF",
                "-DHAVE_PREFLIGHT_TEST_ARCHIVE=OFF",
                "-DHAVE_SCRIPTING_DEFS_TEST_SDK=OFF",
            ],
            0,
        ),
    ] {
        let output = directory.join(mode);
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(directory)
                .arg("-B")
                .arg(&output)
                .arg("-DCMAKE_BUILD_TYPE=Debug")
                .args(options),
        );
        if mode == "enabled" {
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--parallel", "2"]),
            );
            let mut names: Vec<_> = expected()["tests"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            names.push("test_core_module_manifest_source".into());
            for name in names {
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
                assert_cases(&result, &name);
            }
        }
        let result = checked(
            Command::new("ctest")
                .arg("--test-dir")
                .arg(&output)
                .arg("--show-only=json-v1"),
        );
        let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        let tests = inventory["tests"].as_array().unwrap();
        assert_eq!(tests.len(), count, "{mode}");
        for test in tests {
            assert!(
                test["name"] == "test_core_module_manifest_source"
                    || expected()["tests"]
                        .as_object()
                        .unwrap()
                        .contains_key(test["name"].as_str().unwrap())
            );
            assert!(
                !test["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["name"] == "SKIP_RETURN_CODE")
            );
        }
    }
}

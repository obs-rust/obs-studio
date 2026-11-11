use super::*;

#[test]
fn camera_wrap_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let target = &raw["targets"]["test_pipewire_camera_registry_race"];
    let groups = &target["unity_groups"];
    assert_eq!(
        groups["pipewire-implementation"],
        serde_json::json!(["plugins/linux-pipewire/pipewire.c.h"])
    );
    let callers = groups["camera-caller"].as_array().unwrap();
    assert!(
        callers
            .iter()
            .any(|s| s == "test/cmocka/test_pipewire_camera_registry_race.c")
    );
    assert!(
        callers
            .iter()
            .any(|s| s == "plugins/linux-pipewire/camera-portal.c.h")
    );
    assert!(
        !callers
            .iter()
            .any(|s| s == "plugins/linux-pipewire/pipewire.c.h")
    );
    assert_eq!(target["pch"], "cmake/pch/obs-c-std.h");
    let graph = obs_build::validation::graph::Graph::parse(
        &serde_json::json!({
            "schema": 1,
            "targets": {"test_pipewire_camera_registry_race": target},
            "tests": {"test_pipewire_camera_registry_race": raw["tests"]["test_pipewire_camera_registry_race"]}
        }).to_string(),
    ).unwrap();
    for linux in [false, true] {
        for owner in [false, true] {
            for display in [false, true] {
                let selection = graph
                    .select(&std::collections::BTreeMap::from([
                        ("OS_LINUX".into(), linux),
                        ("HAVE_PIPEWIRE_CAMERA_TEST_OWNER".into(), owner),
                        ("HAVE_X11_DISPLAY".into(), display),
                    ]))
                    .unwrap();
                assert_eq!(
                    selection.targets.len(),
                    usize::from(linux && owner && display)
                );
                assert_eq!(selection.tests.len(), selection.targets.len());
            }
        }
    }
}

#[test]
fn native_pulse_server_in_tier2_closure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let packages =
        fs::read_to_string(root.join("rust/tools/linux-validate/apt-packages.txt")).unwrap();
    assert!(packages.lines().any(|line| line == "pulseaudio"));
    let runtime =
        fs::read_to_string(root.join("rust/tools/linux-validate/runtime-packages.py")).unwrap();
    assert!(runtime.contains("'pulseaudio',"));
}

#[cfg(target_os = "linux")]
#[test]
fn unity_partitions_preserve_linker_wrap_in_both_consumers() {
    use obs_build::validation::graph::Graph;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("fixture.h"), "#pragma once\n").unwrap();
    fs::write(
        root.join("caller.c"),
        "int answer(void); int main(void) { return answer() != 7; }\n",
    )
    .unwrap();
    fs::write(
        root.join("wrapper.c"),
        "int __real_answer(void); int __wrap_answer(void) { return __real_answer() + 1; }\n",
    )
    .unwrap();
    fs::write(
        root.join("implementation.c"),
        "int answer(void) { return 6; }\n",
    )
    .unwrap();
    let raw = serde_json::json!({
        "schema": 1,
        "targets": {"wrap": {
            "sources": ["caller.c", "wrapper.c", "implementation.c"],
            "unity_groups": {"callers": ["caller.c", "wrapper.c"], "implementation": ["implementation.c"]},
            "pch": "fixture.h", "link_options": ["-Wl,--wrap=answer"]
        }},
        "tests": {"wrap": {"target": "wrap"}}
    });
    let graph = Graph::parse(&raw.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: root.join("native"),
        profile: Default::default(),
    };
    let artifacts = graph
        .compile(&build, root, &Default::default(), &Default::default())
        .unwrap();
    assert!(
        std::process::Command::new(&artifacts["wrap"])
            .status()
            .unwrap()
            .success()
    );
    fs::write(root.join("graph.json"), raw.to_string()).unwrap();
    fs::write(
        root.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(wrap C)
enable_testing()
include("{repo}/cmake/common/unity.cmake")
include("{repo}/cmake/common/pch.cmake")
include("{repo}/cmake/common/groups.cmake")
include("{repo}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "${{CMAKE_CURRENT_SOURCE_DIR}}")
"#,
            repo = repo.display()
        ),
    )
    .unwrap();
    for args in [vec!["-S", ".", "-B", "cmake"], vec!["--build", "cmake"]] {
        let result = std::process::Command::new("cmake")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert!(
        std::process::Command::new(root.join("cmake/wrap"))
            .status()
            .unwrap()
            .success()
    );
    for (field, value) in [
        (
            "unity_groups",
            serde_json::json!({"callers": ["caller.c", "caller.c"]}),
        ),
        ("pch", serde_json::json!("../outside.h")),
    ] {
        let mut invalid = raw.clone();
        invalid["targets"]["wrap"][field] = value;
        assert!(Graph::parse(&invalid.to_string()).is_err());
    }
}

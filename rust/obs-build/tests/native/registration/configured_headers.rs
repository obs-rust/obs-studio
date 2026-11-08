use super::*;
use std::process::Command;

fn configured_headers_match(native: &str, cmake: &str) -> bool {
    native == cmake
}

#[test]
fn configured_headers_match_with_crlf_template_input() {
    let template = "#define ANSWER (@VALUE@ + 1)\r\n#define OTHER 7\r\n";
    let configured = template.replace("@VALUE@", "41");
    let lf = configured.replace("\r\n", "\n");
    assert!(configured_headers_match(&lf, &configured));
    assert!(configured_headers_match(&configured, &lf));
    assert!(!configured_headers_match(
        &lf,
        &configured.replace("41", "40")
    ));
}

fn checked(command: &mut Command) -> std::process::Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn fixture() -> serde_json::Value {
    serde_json::json!({"schema":1,"targets":{"probe":{
        "sources":["probe.c"],
        "header_libraries":["header-policy"],
        "inherit_header_definitions":true,
        "configured_headers":{"settings.h":{"input":"settings.h.in","values":{"VALUE":"41"}}}
    }},"tests":{"probe":{"target":"probe"}}})
}

#[test]
fn configured_headers_reject_escaping_paths_and_invalid_values() {
    for path in [
        "../outside.h",
        "/outside.h",
        "sub/header.h",
        "C:header.h",
        ".hidden.h",
    ] {
        let mut graph = fixture();
        let header = graph["targets"]["probe"]["configured_headers"]
            .as_object_mut()
            .unwrap()
            .remove("settings.h")
            .unwrap();
        graph["targets"]["probe"]["configured_headers"][path] = header;
        assert!(Graph::parse(&graph.to_string()).is_err(), "{path}");
    }
    for path in [
        "../outside.in",
        "/outside.in",
        "C:/outside.in",
        "a\\b.in",
        "a//b.in",
    ] {
        let mut graph = fixture();
        graph["targets"]["probe"]["configured_headers"]["settings.h"]["input"] = path.into();
        assert!(Graph::parse(&graph.to_string()).is_err(), "{path}");
    }
    for value in ["@OTHER@", "a;b", "a\nb"] {
        let mut graph = fixture();
        graph["targets"]["probe"]["configured_headers"]["settings.h"]["values"]["VALUE"] =
            value.into();
        assert!(Graph::parse(&graph.to_string()).is_err());
    }
}

#[test]
fn configured_headers_match_consumers_and_rebuild_after_template_or_value_changes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("probe.c"), "#include <settings.h>\n#include <stdio.h>\nint main(void) {printf(\"%d\", ANSWER + HEADER_IMPORTED - 7); return 0;}\n").unwrap();
    fs::write(
        root.join("settings.h.in"),
        "#define ANSWER (@VALUE@ + 1)\r\n",
    )
    .unwrap();
    let repository =
        dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    fs::write(
        root.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(configured-header-contract C)
enable_testing()
add_library(header-policy INTERFACE IMPORTED)
set_property(TARGET header-policy PROPERTY INTERFACE_COMPILE_DEFINITIONS HEADER_IMPORTED=7)
include("{}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "${{CMAKE_CURRENT_SOURCE_DIR}}")
"#,
            repository.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: root.join("native"),
        profile: Default::default(),
    };
    let interfaces = std::collections::BTreeMap::from([(
        "header-policy".into(),
        obs_build::validation::graph::Dependency {
            definitions: vec![("HEADER_IMPORTED".into(), Some("7".into()))],
            ..Default::default()
        },
    )]);
    let mut graph = fixture();
    for step in 0..3 {
        if step == 1 {
            fs::write(
                root.join("settings.h.in"),
                "#define ANSWER (@VALUE@ + 2)\r\n",
            )
            .unwrap();
        } else if step == 2 {
            graph["targets"]["probe"]["configured_headers"]["settings.h"]["values"]["VALUE"] =
                "40".into();
        }
        fs::write(root.join("graph.json"), graph.to_string()).unwrap();
        let parsed = Graph::parse(&graph.to_string()).unwrap();
        let artifacts = parsed
            .compile(&build, root, &Default::default(), &interfaces)
            .unwrap();
        let native = checked(&mut Command::new(&artifacts["probe"]));
        let expected = if step == 1 { b"43" } else { b"42" };
        assert_eq!(native.stdout, expected);
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(root)
                .arg("-B")
                .arg(root.join("cmake"))
                .args(["-G", "Ninja"]),
        );
        checked(Command::new("cmake").arg("--build").arg(root.join("cmake")));
        let cmake = checked(&mut Command::new(root.join("cmake/probe")));
        assert_eq!(cmake.stdout, expected);
        checked(
            Command::new("ctest")
                .arg("--test-dir")
                .arg(root.join("cmake"))
                .args(["--no-tests=error", "--output-on-failure"]),
        );
        assert!(configured_headers_match(
            &fs::read_to_string(root.join("native/native-tests/probe/configured/settings.h"))
                .unwrap(),
            &fs::read_to_string(root.join("cmake/native-tests/probe/configured/settings.h"))
                .unwrap()
        ));
    }
}

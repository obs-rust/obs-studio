//! Exercise CMake's actual generated-library edge without a Windows SDK.
use super::*;
use obs_build::validation::graph::Graph;
use std::process::Command;

fn checked(command: &mut Command) -> std::process::Output {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

#[test]
fn parser_data_import_library_is_generated_before_core_linkage() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("import fixture");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("main.c"), "int main(void) { return 0; }\n").unwrap();
    let definition = "LIBRARY obs.dll\nEXPORTS\n gs_effect_get_param_by_name DATA\n";
    fs::write(root.join("lookup.def"), definition).unwrap();
    let librarian = root.join("librarian.sh");
    fs::write(
        &librarian,
        r#"#!/bin/sh
set -eu
for arg do
 case "$arg" in
  /out:*) output=${arg#/out:};;
  /def:*) definition=${arg#/def:};;
 esac
done
cat "$definition" > "$output"
printf '%s\n' "$@" >> "$output"
"#,
    )
    .unwrap();
    checked(Command::new("chmod").arg("+x").arg(&librarian));
    let mut raw = serde_json::json!({"schema":1,
        "targets":{"parser":{"sources":["main.c"],
            "msvc_import_library":{"definition":"lookup.def","output":"lookup.lib"},
            "libraries":["core"]}},
        "tests":{"parser":{"target":"parser"}}});
    Graph::parse(&raw.to_string()).unwrap();
    let consumer =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/cmocka/native-test-graph.cmake");
    fs::write(
        root.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(import-contract C)
enable_testing()
option(IMPORT_MSVC "Generate the DATA-only library" ON)
set(MSVC ${{IMPORT_MSVC}})
set(CMAKE_C_COMPILER_ARCHITECTURE_ID X64)
set(CMAKE_AR "{librarian}")
add_library(core INTERFACE IMPORTED)
include("{consumer}")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "${{CMAKE_CURRENT_SOURCE_DIR}}")
get_target_property(links parser LINK_LIBRARIES)
file(WRITE "${{CMAKE_CURRENT_BINARY_DIR}}/links.txt" "${{links}}")
"#,
            librarian = librarian.display(),
            consumer = consumer.display()
        ),
    )
    .unwrap();
    fs::write(root.join("graph.json"), raw.to_string()).unwrap();
    for msvc in [true, false] {
        let output = root.join(if msvc { "msvc output" } else { "non-msvc" });
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(&root)
                .arg("-B")
                .arg(&output)
                .args(["-G", "Ninja"])
                .arg(format!("-DIMPORT_MSVC={}", if msvc { "ON" } else { "OFF" })),
        );
        let links = fs::read_to_string(output.join("links.txt")).unwrap();
        if msvc {
            assert_eq!(
                links,
                format!("{};core", output.join("lookup.lib").display())
            );
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--target", "lookup.lib"]),
            );
            let library = fs::read_to_string(output.join("lookup.lib")).unwrap();
            assert!(library.starts_with(definition));
            assert!(library.contains("/machine:X64\n"));
            assert!(library.contains(&format!("/def:{}\n", root.join("lookup.def").display())));
            assert!(library.contains(&format!("/out:{}\n", output.join("lookup.lib").display())));
        } else {
            assert_eq!(links, "core");
            assert!(!output.join("lookup.lib").exists());
        }
    }
    assert_invalid_imports(&root, &mut raw);
}

fn assert_invalid_imports(root: &Path, raw: &mut serde_json::Value) {
    for (field, invalid) in [("output", "../escape.lib"), ("definition", "../lookup.def")] {
        let original = raw["targets"]["parser"]["msvc_import_library"][field].clone();
        raw["targets"]["parser"]["msvc_import_library"][field] = invalid.into();
        assert!(
            Graph::parse(&raw.to_string())
                .unwrap_err()
                .contains("MSVC import library")
        );
        fs::write(root.join("graph.json"), raw.to_string()).unwrap();
        assert!(
            !Command::new("cmake")
                .arg("-S")
                .arg(root)
                .arg("-B")
                .arg(root.join(format!("invalid-{field}")))
                .output()
                .unwrap()
                .status
                .success()
        );
        raw["targets"]["parser"]["msvc_import_library"][field] = original;
    }
}

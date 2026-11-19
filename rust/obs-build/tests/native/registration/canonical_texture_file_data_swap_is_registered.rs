use super::*;

#[test]
fn canonical_texture_file_data_swap_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_texture_file_data_steps_rust";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(
                &[
                    (os.into(), true),
                    ("OS_FREEBSD".into(), false),
                    ("HAVE_LIBOBS_RUST_STATIC".into(), true),
                ]
                .into(),
            )
            .unwrap();
        assert_eq!(selected.tests[name].target, name);
        assert!(format!("{:?}", selected.targets[name]).contains("libobs_rust-static"));
    }
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-texture-file-data+.h.c")).unwrap();
    assert!(group.contains("libobs/graphics/graphics-texture-file-data.c.h"));
    let media = fs::read_to_string(root.join("libobs/build/libobs.media+.h.c")).unwrap();
    assert!(!media.contains("libobs/graphics/graphics-texture-file-data.c.h"));
}

#[test]
fn standalone_texture_file_data_adapters_supply_private_max_size_callback() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let adapter = "libobs/build/libobs.media-texture-file-data-adapter+.h.c";
    let callback = "test/cmocka/texture-file-data-max-size-link-stub.c";
    let mut consumers = 0;
    for (name, target) in graph["targets"].as_object().unwrap() {
        let sources = target["sources"].as_array().unwrap();
        if sources.contains(&serde_json::json!(adapter)) {
            consumers += 1;
            assert!(sources.contains(&serde_json::json!(callback)), "{name}");
        }
    }
    assert!(consumers > 0);
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/texture_file_data_link_support.cmake"))
            .unwrap();
    assert!(fragment.contains(callback));
}

#[test]
fn standalone_image_adapter_supplies_windows_host_imports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let graph = serde_json::json!({"targets": {
        "test_video_stop_requested_rust": graph["targets"]["test_video_stop_requested_rust"]
    }});
    let root = root.to_string_lossy().replace('\\', "/");
    for (system, windows) in [("Windows", true), ("Linux", false)] {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("probe.c"),
            "int main(void) { return 0; }\n",
        )
        .unwrap();
        fs::write(directory.path().join("graph.json"), graph.to_string()).unwrap();
        fs::write(
            directory.path().join("CMakeLists.txt"),
            format!(
                r##"cmake_minimum_required(VERSION 3.28)
project(image-adapter-link-contract C)
set(CMAKE_SYSTEM_NAME "{system}")
set(CMAKE_SOURCE_DIR "{root}")
set(OS_WINDOWS {windows})
foreach(dependency IN ITEMS OBS::libobs FFmpeg::avcodec FFmpeg::avformat FFmpeg::avutil FFmpeg::swscale)
  add_library(${{dependency}} INTERFACE IMPORTED)
endforeach()
add_library(libobs_rust-static INTERFACE)
foreach(consumer IN ITEMS test_video_stop_requested_rust test_texture_file_data_steps_rust)
  add_executable(${{consumer}} "${{CMAKE_CURRENT_SOURCE_DIR}}/probe.c")
  target_link_libraries(${{consumer}} PRIVATE libobs_rust-static)
endforeach()
add_executable(reference_consumer "${{CMAKE_CURRENT_SOURCE_DIR}}/probe.c")
include("{root}/test/cmocka/native-test-graph.cmake")
file(READ "${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" graph)
_obs_test_graph_sources(video_sources "${{graph}}" test_video_stop_requested_rust)
if("libobs/util/base-variadic.c.h" IN_LIST video_sources)
  file(WRITE "${{CMAKE_CURRENT_BINARY_DIR}}/base-variadic-provider.c" "#include \"{root}/libobs/util/base-variadic.c.h\"\n")
  target_sources(test_video_stop_requested_rust PRIVATE "${{CMAKE_CURRENT_BINARY_DIR}}/base-variadic-provider.c")
endif()
include("{root}/test/cmocka/tests.d/texture_file_data_link_support.cmake")
"##
            ),
        )
        .unwrap();
        let build = directory.path().join("build");
        let configure = std::process::Command::new("cmake")
            .arg("-S")
            .arg(directory.path())
            .arg("-B")
            .arg(&build)
            .args(["-G", "Ninja"])
            .output()
            .unwrap();
        assert!(
            configure.status.success(),
            "{}",
            String::from_utf8_lossy(&configure.stderr)
        );
        for consumer in [
            "test_video_stop_requested_rust",
            "test_texture_file_data_steps_rust",
            "reference_consumer",
        ] {
            let commands = std::process::Command::new("cmake")
                .arg("--build")
                .arg(&build)
                .args(["--target", consumer, "--", "-n", "-v"])
                .output()
                .unwrap();
            assert!(commands.status.success());
            let commands = String::from_utf8_lossy(&commands.stdout);
            let lower = commands.to_ascii_lowercase();
            let links = |library| {
                lower.split_whitespace().any(|token| {
                    let token = token.trim_matches('"');
                    token == format!("-l{library}") || token == format!("{library}.lib")
                })
            };
            assert_eq!(
                (
                    links("winmm"),
                    links("rpcrt4"),
                    commands.contains("base-variadic-provider.c")
                ),
                (
                    windows && consumer != "reference_consumer",
                    windows && consumer != "reference_consumer",
                    windows && consumer == "test_video_stop_requested_rust"
                ),
                "{system}/{consumer}: {commands}"
            );
        }
    }
}

#[test]
fn standalone_graphics_fixtures_bind_the_static_implementation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    for name in [
        "test_texture_file_data_steps_rust",
        "test_effect_parser_lifecycle_rust",
    ] {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("shared.c"),
            "int implementation(void) { return 1; }\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("static.c"),
            "int implementation(void) { return 0; }\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("main.c"),
            "int implementation(void); int main(void) { return implementation(); }\n",
        )
        .unwrap();
        let libraries: Vec<_> = graph["targets"][name]["libraries"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|library| match library.as_str() {
                Some("OBS::libobs") => Some("shared_provider"),
                Some("libobs_rust-static") => Some("static_provider"),
                _ => None,
            })
            .collect();
        fs::write(
            directory.path().join("CMakeLists.txt"),
            format!(
                r#"cmake_minimum_required(VERSION 3.28)
project(static-implementation-binding C)
set(CMAKE_WINDOWS_EXPORT_ALL_SYMBOLS ON)
add_library(shared_provider SHARED shared.c)
add_library(static_provider STATIC static.c)
add_executable(probe main.c)
target_link_libraries(probe PRIVATE {})
"#,
                libraries.join(" ")
            ),
        )
        .unwrap();
        let output = std::process::Command::new("cmake")
            .args(["-S", ".", "-B", "build", "-G", "Ninja"])
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = std::process::Command::new("cmake")
            .args(["--build", "build"])
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let status = std::process::Command::new(
            directory
                .path()
                .join(format!("build/probe{}", std::env::consts::EXE_SUFFIX)),
        )
        .status()
        .unwrap();
        assert!(
            status.success(),
            "{name} selected shared libobs ahead of the Rust archive"
        );
    }
}

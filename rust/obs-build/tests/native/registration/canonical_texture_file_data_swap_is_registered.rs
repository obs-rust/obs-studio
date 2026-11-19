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
fn standalone_image_adapter_supplies_windows_timer_imports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = root.to_string_lossy().replace('\\', "/");
    for (system, windows) in [("Windows", true), ("Linux", false)] {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("probe.c"),
            "int main(void) { return 0; }\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("CMakeLists.txt"),
            format!(
                r#"cmake_minimum_required(VERSION 3.28)
project(image-adapter-link-contract C)
set(CMAKE_SYSTEM_NAME "{system}")
set(CMAKE_SOURCE_DIR "{root}")
foreach(dependency IN ITEMS OBS::libobs FFmpeg::avcodec FFmpeg::avformat FFmpeg::avutil FFmpeg::swscale)
  add_library(${{dependency}} INTERFACE IMPORTED)
endforeach()
add_library(libobs_rust-static INTERFACE)
foreach(consumer IN ITEMS test_video_stop_requested_rust test_texture_file_data_steps_rust)
  add_executable(${{consumer}} "${{CMAKE_CURRENT_SOURCE_DIR}}/probe.c")
  target_link_libraries(${{consumer}} PRIVATE libobs_rust-static)
endforeach()
add_executable(reference_consumer "${{CMAKE_CURRENT_SOURCE_DIR}}/probe.c")
include("{root}/test/cmocka/tests.d/texture_file_data_link_support.cmake")
"#
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
            assert_eq!(
                commands.contains("winmm"),
                windows && consumer != "reference_consumer",
                "{system}/{consumer}: {commands}"
            );
        }
    }
}

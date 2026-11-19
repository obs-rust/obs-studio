use super::*;

#[test]
fn canonical_video_info_query_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let c = &graph.targets["test_video_info_query"];
    let rust = &graph.targets["test_video_info_query_rust"];
    assert!(
        matches!(&c.sources[0], Source::Path(path) if path == "test/cmocka/test_video_info_query.c")
    );
    assert!(
        matches!(&rust.sources[0], Source::Path(path) if path == "test/cmocka/test_video_info_query.c")
    );
    assert!(
        matches!(&c.sources[1], Source::Path(path) if path == "libobs/obs-video-info-query.c.h")
    );
    assert!(
        matches!(&rust.sources[1], Source::Path(path) if path == "test/cmocka/video-info-query-link-support.c")
    );
    graph
        .targets
        .retain(|name, _| name.starts_with("test_video_info_query"));
    graph
        .tests
        .retain(|name, _| name.starts_with("test_video_info_query"));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[(os.into(), true), ("HAVE_LIBOBS_RUST_STATIC".into(), true)].into())
            .unwrap();
        for name in ["test_video_info_query", "test_video_info_query_rust"] {
            assert!(selected.targets.contains_key(name), "{os}: {name}");
            assert!(selected.tests.contains_key(name), "{os}: {name}");
        }
        assert!(
            selected.targets["test_video_info_query_rust"]
                .libraries
                .iter()
                .any(|lib| matches!(lib, obs_build::validation::graph::ConditionalValue::Name(name) if name == "libobs_rust-static"))
        );
    }
    let group = fs::read_to_string(root.join("libobs/build/libobs.core+.h.c")).unwrap();
    assert!(group.contains("obs-video-info-query-fields.c.h"));
    assert!(!group.contains("\"libobs/obs-video-info-query.c.h\""));
}

/// Private adapters are hidden in shared libobs, so linking it must not exclude
/// a Rust-static consumer from the demand-linked real-adapter support archive.
#[test]
fn video_info_query_support_covers_all_rust_static_consumers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path();
    fs::write(source.join("main.c"), "int main(void) { return 0; }\n").unwrap();
    fs::write(
        source.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(video_info_query_support C)
set(CMAKE_SOURCE_DIR "{root}")
set(CMAKE_CURRENT_SOURCE_DIR "{root}/test/cmocka")
add_library(libobs_rust-static INTERFACE)
add_library(OBS::libobs INTERFACE IMPORTED)
add_library(OBS::caption INTERFACE IMPORTED)
add_library(Uthash::Uthash INTERFACE IMPORTED)
add_library(OBS::w32-pthreads INTERFACE IMPORTED)
foreach(name IN ITEMS standalone with_shared future_consumer reference)
  add_executable(${{name}} "{source}/main.c")
  if(NOT name STREQUAL "reference")
    target_link_libraries(${{name}} PRIVATE libobs_rust-static)
  endif()
endforeach()
target_link_libraries(with_shared PRIVATE OBS::libobs)
target_link_libraries(reference PRIVATE OBS::libobs)
include("{root}/test/cmocka/tests.d/video_info_query_link_support.cmake")
function(check_support)
  get_target_property(adapter_sources video_info_query_link_support SOURCES)
  foreach(adapter IN ITEMS encoder-video-tex-active-fields encoder-video-tex-active-mix encoder-get-name
                          output-interleave-name source-audio-capture-callback)
    if(NOT "{root}/test/cmocka/${{adapter}}-link-support.c" IN_LIST adapter_sources)
      message(FATAL_ERROR "missing real private adapter source: ${{adapter}}")
    endif()
  endforeach()
  foreach(name IN ITEMS standalone with_shared future_consumer reference)
    get_target_property(links ${{name}} LINK_LIBRARIES)
    if(name STREQUAL "reference")
      if("video_info_query_link_support" IN_LIST links)
        message(FATAL_ERROR "${{name}}: unexpected Rust adapter support")
      endif()
    elseif(NOT "video_info_query_link_support" IN_LIST links)
      message(FATAL_ERROR "${{name}}: missing private video-info adapters")
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL check_support)
"#,
            root = dunce::canonicalize(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
            source = source.to_string_lossy().replace('\\', "/"),
        ),
    )
    .unwrap();
    let output = std::process::Command::new("cmake")
        .arg("-S")
        .arg(source)
        .arg("-B")
        .arg(source.join("build"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_startup_cycle_stress",
    "test_video_io_hidden",
    "test_gl_dynamic_texture",
    "test_nix_startup_log",
    "test_scene_size_race",
    "test_startup_alloc_stable",
];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("display_startup_expected.json")).unwrap()
}
#[test]
fn canonical_display_startup_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let expected = expected();
    for name in CASES {
        for section in ["targets", "tests"] {
            assert_eq!(
                raw[section][name], expected[section][name],
                "{section}/{name}"
            );
        }
        let fragment_name = if name == "test_video_io_hidden" {
            "test_video_io"
        } else {
            name
        };
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{fragment_name}.cmake")))
                .unwrap();
        if name == "test_video_io_hidden" {
            assert!(!fragment.contains("add_executable(test_video_io_hidden"));
            assert!(!fragment.contains("add_test(test_video_io_hidden"));
        } else {
            assert!(!fragment.contains("add_executable("), "{name}");
            assert!(!fragment.contains("add_test("), "{name}");
        }
        assert!(fragment.contains("test-graph.d"));
    }
}
#[test]
fn canonical_display_startup_guards() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    for os in ["OS_LINUX", "OS_WINDOWS", "OS_MACOS"] {
        for display in [None, Some(false), Some(true)] {
            for renderer in [false, true] {
                let mut facts = std::collections::BTreeMap::from([
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("HAVE_OBS_OPENGL".into(), renderer),
                ]);
                if let Some(display) = display {
                    facts.insert("HAVE_X11_DISPLAY".into(), display);
                }
                let selected = graph.select(&facts).unwrap();
                for name in CASES {
                    let enabled = if matches!(
                        name,
                        "test_startup_alloc_stable"
                            | "test_startup_cycle_stress"
                            | "test_video_io_hidden"
                    ) {
                        os != "OS_LINUX" || display == Some(true)
                    } else {
                        os == "OS_LINUX"
                            && display == Some(true)
                            && (renderer || name != "test_gl_dynamic_texture")
                    };
                    assert_eq!(
                        selected.targets.contains_key(name),
                        enabled,
                        "{os}/{display:?}/{renderer}/{name}"
                    );
                    assert_eq!(
                        selected.tests.contains_key(name),
                        enabled,
                        "{os}/{display:?}/{renderer}/{name}"
                    );
                }
            }
        }
    }
    let typo = Graph::parse(
        r#"{"schema":1,"targets":{"test":{"sources":["test.c"],"requires":"HAVE_X11_DISPALY"}},"tests":{"test":{"target":"test"}}}"#,
    )
    .unwrap();
    assert!(
        typo.select(&Default::default())
            .err()
            .unwrap()
            .contains("HAVE_X11_DISPALY")
    );
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/display_startup_runtime.rs"]
mod runtime;

#[test]
fn canonical_display_startup_declared_runner() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = tempfile::tempdir().unwrap();
    let script = directory.path().join("display.cmake");
    let consumer = root
        .join("test/cmocka/native-test-graph.cmake")
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        &script,
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
include("{consumer}")
foreach(linux IN ITEMS TRUE FALSE)
  set(OS_LINUX ${{linux}})
  unset(HAVE_X11_DISPLAY)
  unset(OBS_XVFB_RUN_REAL)
  unset(XVFB_RUN)
  _obs_test_graph_x11_display(selected)
  if(selected)
    message(FATAL_ERROR "No server was declared")
  endif()
  foreach(runner IN ITEMS OBS_XVFB_RUN_REAL XVFB_RUN)
    set(${{runner}} "/declared/xvfb-run")
    _obs_test_graph_x11_display(selected)
    if(NOT selected STREQUAL linux)
      message(FATAL_ERROR "Declared runner lost Linux guard")
    endif()
    set(HAVE_X11_DISPLAY FALSE)
    _obs_test_graph_x11_display(selected)
    if(selected)
      message(FATAL_ERROR "Explicit false declaration ignored")
    endif()
    unset(HAVE_X11_DISPLAY)
    unset(${{runner}})
  endforeach()
endforeach()
set(OS_LINUX TRUE)
set(ENV{{DISPLAY}} ":123")
_obs_test_graph_x11_display(selected)
if(NOT selected)
  message(FATAL_ERROR "Declared display ignored")
endif()
"#
        ),
    )
    .unwrap();
    let output = std::process::Command::new("cmake")
        .arg("-P")
        .arg(&script)
        .env_remove("DISPLAY")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn has_skipped_cases(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    ["skipped ]", "skipped test", "skipped:", "skipping"]
        .iter()
        .any(|marker| text.contains(marker))
}

#[test]
fn canonical_display_startup_skip_detection() {
    assert!(!has_skipped_cases(
        "info: Video stopped, number of skipped frames due to encoding lag: 1/1 (100.0%)\n[  PASSED  ] 4 test(s)."
    ));
    for output in [
        "[  SKIPPED ] test_startup_shutdown_cycles",
        "1 SKIPPED TEST(S)",
        "skipped: no display",
        "core video failed; skipping",
    ] {
        assert!(has_skipped_cases(output), "{output}");
    }
}

use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_text_file",
    "test_v4l2_vcam_write",
    "test_v4l2_vcam_audio",
    "test_v4l2_fd_cloexec",
    "test_hotkey_background_ui",
    "test_hotkey_background_use_key",
];
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/headless_helpers_runtime.rs"]
mod runtime;
fn expected() -> serde_json::Value {
    serde_json::json!({"schema": 1, "targets": {"test_text_file": {"sources": ["test/cmocka/test_text_file.c", "plugins/text-freetype2/text-file.c.h"], "libraries": ["OBS::libobs", "OBS::native-test-cmocka"], "requires": "HAVE_TEXT_FILE_OWNER", "includes": ["plugins/text-freetype2"], "watch_inputs": ["plugins/text-freetype2/build/text-freetype2.pch.h"], "header_libraries": ["Freetype::Freetype"]}, "test_v4l2_vcam_write": {"sources": ["test/cmocka/test_v4l2_vcam_write.c", "plugins/linux-v4l2/vcam-format.c.h"], "libraries": ["OBS::libobs", "OBS::native-test-cmocka"], "requires": {"all": ["OS_LINUX", "HAVE_OBS_V4L2"]}, "includes": [".", "plugins/linux-v4l2"], "link_options": ["-Wl,--wrap=write"], "watch_inputs": ["plugins/linux-v4l2/v4l2-output.c.h", "plugins/linux-v4l2/build/linux-v4l2-nix.pch.h", "plugins/linux-v4l2/build/linux-v4l2-nix-output.pch.h"], "compile_options": ["-fvisibility=default"], "header_libraries": ["Libv4l2::Libv4l2"]}, "test_v4l2_vcam_audio": {"sources": ["test/cmocka/test_v4l2_vcam_audio.c", "plugins/linux-v4l2/vcam-format.c.h"], "libraries": ["OBS::libobs", "OBS::native-test-cmocka"], "requires": {"all": ["OS_LINUX", "HAVE_OBS_V4L2"]}, "includes": [".", "plugins/linux-v4l2"], "watch_inputs": ["plugins/linux-v4l2/v4l2-output.c.h", "plugins/linux-v4l2/build/linux-v4l2-nix.pch.h", "plugins/linux-v4l2/build/linux-v4l2-nix-output.pch.h"], "header_libraries": ["Libv4l2::Libv4l2"]}, "test_v4l2_fd_cloexec": {"sources": ["test/cmocka/test_v4l2_fd_cloexec.c", "plugins/linux-v4l2/v4l2-busy.c.h", "plugins/linux-v4l2/v4l2-controls.c.h", "plugins/linux-v4l2/v4l2-decoder.c.h", "plugins/linux-v4l2/v4l2-frame-action.c.h", "plugins/linux-v4l2/v4l2-framerate.c.h", "plugins/linux-v4l2/v4l2-helpers.c.h", "plugins/linux-v4l2/v4l2-input-caps.c.h", "plugins/linux-v4l2/v4l2-input.c.h"], "libraries": ["OBS::libobs", "FFmpeg::avcodec", "FFmpeg::avformat", "FFmpeg::avutil", "OBS::native-test-cmocka"], "requires": {"all": ["OS_LINUX", "HAVE_OBS_V4L2"]}, "includes": ["plugins/linux-v4l2"], "watch_inputs": ["plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"], "header_libraries": ["Libv4l2::Libv4l2"]}, "test_hotkey_background_ui": {"sources": ["test/cmocka/test_hotkey_background_ui.cpp", "frontend/settings/OBSHotkeyBackground.cpp.hpp"], "libraries": ["OBS::libobs", "nlohmann_json::nlohmann_json", "OBS::native-test-cmocka"], "requires": {"all": [{"any": ["OS_LINUX", "OS_FREEBSD"]}, "HAVE_HOTKEY_BACKGROUND_JSON"]}, "includes": ["frontend/settings"], "definitions": ["HOTKEY_LOCALE_EN_US=\"${source}/frontend/data/locale/en-US.ini\""], "watch_inputs": ["frontend/build/frontend-settings-obshotkeybackground.pch.hpp", "frontend/data/locale/en-US.ini"], "cpp_standard": "17"}, "test_hotkey_background_use_key": {"sources": ["test/cmocka/test_hotkey_background_use_key.cpp", "frontend/settings/OBSHotkeyBackground.cpp.hpp"], "libraries": ["OBS::libobs", "nlohmann_json::nlohmann_json", "OBS::native-test-cmocka"], "requires": {"all": [{"any": ["OS_LINUX", "OS_FREEBSD"]}, "HAVE_HOTKEY_BACKGROUND_JSON"]}, "includes": ["frontend/settings"], "watch_inputs": ["frontend/build/frontend-settings-obshotkeybackground.pch.hpp"], "cpp_standard": "17"}}, "tests": {"test_text_file": {"target": "test_text_file", "requires": "HAVE_TEXT_FILE_OWNER", "command": ["${target}"]}, "test_v4l2_vcam_write": {"target": "test_v4l2_vcam_write", "requires": {"all": ["OS_LINUX", "HAVE_OBS_V4L2"]}, "command": ["${target}"], "timeout_seconds": 60}, "test_v4l2_vcam_audio": {"target": "test_v4l2_vcam_audio", "requires": {"all": ["OS_LINUX", "HAVE_OBS_V4L2"]}, "command": ["${target}"], "timeout_seconds": 60}, "test_v4l2_fd_cloexec": {"target": "test_v4l2_fd_cloexec", "requires": {"all": ["OS_LINUX", "HAVE_OBS_V4L2"]}, "command": ["${target}"], "timeout_seconds": 60}, "test_hotkey_background_ui": {"target": "test_hotkey_background_ui", "requires": {"all": [{"any": ["OS_LINUX", "OS_FREEBSD"]}, "HAVE_HOTKEY_BACKGROUND_JSON"]}, "command": ["${target}"]}, "test_hotkey_background_use_key": {"target": "test_hotkey_background_use_key", "requires": {"all": [{"any": ["OS_LINUX", "OS_FREEBSD"]}, "HAVE_HOTKEY_BACKGROUND_JSON"]}, "command": ["${target}"]}}})
}
#[test]
fn canonical_headless_helpers_is_registered() {
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
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
        assert!(fragment.contains("test-graph.d"));
    }
}
#[test]
fn canonical_headless_helpers_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.targets.retain(|n, _| CASES.contains(&n.as_str()));
    graph.tests.retain(|n, _| CASES.contains(&n.as_str()));
    assert_eq!(graph.targets.len(), 6);
    for os in ["OS_LINUX", "OS_MACOS", "OS_FREEBSD", "OS_WINDOWS"] {
        for owner in [false, true] {
            for json in [false, true] {
                let facts = [
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_FREEBSD".into(), os == "OS_FREEBSD"),
                    ("HAVE_OBS_V4L2".into(), owner),
                    ("HAVE_TEXT_FILE_OWNER".into(), owner),
                    ("HAVE_HOTKEY_BACKGROUND_JSON".into(), json),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                let count = usize::from(owner)
                    + 3 * usize::from(owner && os == "OS_LINUX")
                    + 2 * usize::from(json && ["OS_LINUX", "OS_FREEBSD"].contains(&os));
                assert_eq!(selected.targets.len(), count, "{os}/{owner}/{json}");
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}

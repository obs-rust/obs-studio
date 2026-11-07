use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 6] = ["test_hotkey_binding_getters", "test_module_failure_info", "test_scene_item_state_getters", "test_pulse_timing", "test_ffmpeg_encoder_missing", "test_ffmpeg_output_connecting"];

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "headless_abi_runtime.rs"]
mod runtime;

fn expected() -> serde_json::Value { serde_json::json!({"schema": 1, "targets": {"test_hotkey_binding_getters": {"sources": ["test/cmocka/test_hotkey_binding_getters.c"], "libraries": ["OBS::libobs", "OBS::caption", {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}, "OBS::native-test-cmocka"]}, "test_module_failure_info": {"sources": ["test/cmocka/test_module_failure_info.c"], "libraries": ["OBS::libobs", "OBS::native-test-cmocka"]}, "test_scene_item_state_getters": {"sources": ["test/cmocka/test_scene_item_state_getters.c"], "libraries": ["OBS::libobs", {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}, "OBS::native-test-cmocka"], "includes": ["libobs"]}, "test_pulse_timing": {"sources": ["test/cmocka/test_pulse_timing.c", "plugins/linux-pulseaudio/pulse-timing.c.h"], "libraries": ["OBS::libobs", "OBS::native-test-cmocka"], "includes": [".", "plugins/linux-pulseaudio"], "watch_inputs": ["plugins/linux-pulseaudio/build/linux-pulseaudio-nix.pch.h"], "requires": "HAVE_OBS_PULSEAUDIO"}, "test_ffmpeg_encoder_missing": {"sources": ["test/cmocka/test_ffmpeg_encoder_missing.c", "shared/opts-parser/opts-parser.c.h"], "libraries": ["OBS::libobs", "FFmpeg::avcodec", "FFmpeg::avformat", "FFmpeg::avutil", "OBS::native-test-cmocka"], "includes": [".", "shared/opts-parser"], "watch_inputs": ["plugins/obs-ffmpeg/obs-ffmpeg-video-encoders.c.h", "plugins/obs-ffmpeg/build/obs-ffmpeg-video-encoders.pch.h"], "requires": "OS_LINUX"}, "test_ffmpeg_output_connecting": {"sources": ["test/cmocka/test_ffmpeg_output_connecting.c"], "libraries": ["OBS::libobs", "FFmpeg::avcodec", "FFmpeg::avformat", "FFmpeg::avutil", "FFmpeg::swscale", "FFmpeg::swresample", "OBS::native-test-cmocka"], "includes": [".", "plugins/obs-ffmpeg"], "watch_inputs": ["plugins/obs-ffmpeg/obs-ffmpeg-output.c.h", "plugins/obs-ffmpeg/build/obs-ffmpeg-output.pch.h"], "requires": "OS_LINUX"}}, "tests": {"test_hotkey_binding_getters": {"target": "test_hotkey_binding_getters"}, "test_module_failure_info": {"target": "test_module_failure_info", "timeout_seconds": 30}, "test_scene_item_state_getters": {"target": "test_scene_item_state_getters"}, "test_pulse_timing": {"target": "test_pulse_timing", "requires": "HAVE_OBS_PULSEAUDIO"}, "test_ffmpeg_encoder_missing": {"target": "test_ffmpeg_encoder_missing", "timeout_seconds": 60, "requires": "OS_LINUX"}, "test_ffmpeg_output_connecting": {"target": "test_ffmpeg_output_connecting", "timeout_seconds": 60, "requires": "OS_LINUX"}}}) }

/// Refs #1390: preserve six unchanged headless tests and their original contracts.
#[test]
fn canonical_headless_abi_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap()).unwrap();
    let expected = expected();
    for name in CASES {
        for section in ["targets", "tests"] { assert_eq!(raw[section][name], expected[section][name], "{section}/{name}"); }
        let fragment = fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
        assert!(fragment.contains("test-graph.json"));
    }
}

#[test]
fn canonical_headless_abi_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.targets.retain(|name, _| CASES.contains(&name.as_str()));
    graph.tests.retain(|name, _| CASES.contains(&name.as_str()));
    assert_eq!(graph.targets.len(), CASES.len());
    for os in ["OS_LINUX", "OS_MACOS", "OS_FREEBSD", "OS_WINDOWS"] {
        for pulse in [false, true] {
            let facts = [("OS_LINUX".into(), os == "OS_LINUX"), ("OS_WINDOWS".into(), os == "OS_WINDOWS"), ("HAVE_OBS_PULSEAUDIO".into(), pulse)].into();
            let selected = graph.select(&facts).unwrap();
            let expected = 3 + usize::from(pulse) + if os == "OS_LINUX" { 2 } else { 0 };
            assert_eq!(selected.targets.len(), expected, "{os}/{pulse}");
            assert_eq!(selected.tests.len(), expected, "{os}/{pulse}");
        }
    }
}

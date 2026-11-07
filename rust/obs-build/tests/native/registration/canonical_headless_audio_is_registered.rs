use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 6] = [
    "test_alsa_capture_format",
    "test_alsa_device_lost",
    "test_alsa_early_stop",
    "test_alsa_reopen_destroy",
    "test_jack_input",
    "test_ffmpeg_mux_helper",
];

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "headless_audio_runtime.rs"]
mod runtime;

fn expected() -> serde_json::Value {
    serde_json::json!({"schema": 1, "targets": {"test_alsa_capture_format": {"sources": ["test/cmocka/test_alsa_capture_format.c", "plugins/linux-alsa/alsa-input.c.h"], "libraries": ["OBS::libobs", "ALSA::ALSA", "OBS::native-test-cmocka"], "requires": "HAVE_OBS_ALSA", "watch_inputs": ["plugins/linux-alsa/build/linux-alsa-nix.pch.h"], "link_options": ["-Wl,--wrap=obs_source_set_async_compensation,--wrap=obs_source_output_audio,--wrap=snd_pcm_hw_params_test_format,--wrap=snd_pcm_readi"]}, "test_alsa_device_lost": {"sources": ["test/cmocka/test_alsa_device_lost.c", "plugins/linux-alsa/alsa-input.c.h"], "libraries": ["OBS::libobs", "ALSA::ALSA", "OBS::native-test-cmocka"], "requires": "HAVE_OBS_ALSA", "watch_inputs": ["plugins/linux-alsa/build/linux-alsa-nix.pch.h"], "link_options": ["-Wl,--wrap=obs_source_set_async_compensation,--wrap=obs_source_output_audio,--wrap=snd_pcm_open,--wrap=snd_pcm_readi"]}, "test_alsa_early_stop": {"sources": ["test/cmocka/test_alsa_early_stop.c", "plugins/linux-alsa/alsa-input.c.h"], "libraries": ["OBS::libobs", "ALSA::ALSA", "OBS::native-test-cmocka"], "requires": "HAVE_OBS_ALSA", "watch_inputs": ["plugins/linux-alsa/build/linux-alsa-nix.pch.h"], "link_options": ["-Wl,--wrap=obs_source_set_async_compensation,--wrap=obs_source_output_audio,--wrap=pthread_create,--wrap=pthread_join"], "compile_options": ["-fvisibility=default"]}, "test_alsa_reopen_destroy": {"sources": ["test/cmocka/test_alsa_reopen_destroy.c", "plugins/linux-alsa/alsa-input.c.h"], "libraries": ["OBS::libobs", "ALSA::ALSA", "OBS::native-test-cmocka"], "requires": "HAVE_OBS_ALSA", "watch_inputs": ["plugins/linux-alsa/build/linux-alsa-nix.pch.h"], "link_options": ["-Wl,--wrap=obs_source_set_async_compensation,--wrap=obs_source_output_audio"]}, "test_jack_input": {"sources": ["test/cmocka/test_jack_input.c"], "libraries": ["OBS::libobs", "OBS::native-test-cmocka"], "requires": "OS_LINUX", "includes": ["test/cmocka/jack-stubs", ".", "plugins/linux-jack"], "compile_options": [{"name": "-Wno-error=sign-compare", "requires": "C_COMPILER_CLANG"}], "watch_inputs": ["plugins/linux-jack/jack-input.c.h", "plugins/linux-jack/jack-wrapper.c.h", "plugins/linux-jack/build/linux-jack-nix.pch.h"], "link_options": ["-Wl,--wrap=obs_source_get_name,--wrap=obs_source_set_async_compensation,--wrap=obs_source_output_audio"]}, "test_ffmpeg_mux_helper": {"sources": ["test/cmocka/test_ffmpeg_mux_helper.c"], "libraries": ["OBS::libobs", "FFmpeg::avcodec", "FFmpeg::avformat", "FFmpeg::avutil", "OBS::native-test-cmocka"], "requires": "OS_LINUX", "includes": ["."], "watch_inputs": ["plugins/obs-ffmpeg/ffmpeg-mux/ffmpeg-mux.c.h", "plugins/obs-ffmpeg/ffmpeg-mux/build/ffmpeg-mux.pch.h"]}}, "tests": {"test_alsa_capture_format": {"target": "test_alsa_capture_format", "requires": "HAVE_OBS_ALSA", "timeout_seconds": 60}, "test_alsa_device_lost": {"target": "test_alsa_device_lost", "requires": "HAVE_OBS_ALSA", "timeout_seconds": 120}, "test_alsa_early_stop": {"target": "test_alsa_early_stop", "requires": "HAVE_OBS_ALSA", "timeout_seconds": 60}, "test_alsa_reopen_destroy": {"target": "test_alsa_reopen_destroy", "requires": "HAVE_OBS_ALSA", "timeout_seconds": 60}, "test_jack_input": {"target": "test_jack_input", "requires": "OS_LINUX", "timeout_seconds": 60}, "test_ffmpeg_mux_helper": {"target": "test_ffmpeg_mux_helper", "requires": "OS_LINUX", "timeout_seconds": 60}}})
}

/// Refs #1390: preserve six unchanged headless tests and their original contracts.
#[test]
fn canonical_headless_audio_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
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
        assert!(fragment.contains("test-graph.json"));
    }
}

#[test]
fn canonical_headless_audio_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph
        .targets
        .retain(|name, _| CASES.contains(&name.as_str()));
    graph.tests.retain(|name, _| CASES.contains(&name.as_str()));
    assert_eq!(graph.targets.len(), CASES.len());
    for os in ["OS_LINUX", "OS_MACOS", "OS_FREEBSD", "OS_WINDOWS"] {
        for owner in [false, true] {
            let facts = [
                ("OS_LINUX".into(), os == "OS_LINUX"),
                ("HAVE_OBS_ALSA".into(), owner),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            let expected = usize::from(owner) * 4 + if os == "OS_LINUX" { 2 } else { 0 };
            assert_eq!(selected.targets.len(), expected, "{os}/{owner}");
            assert_eq!(selected.tests.len(), expected, "{os}/{owner}");
        }
    }
}

use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_media_format_change",
    "test_media_loop_start_time",
    "test_media_ts_start_time",
    "test_media_seek_keyframe",
    "test_media_alpha_network",
    "test_media_network_timeout",
];
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/media_clips_runtime.rs"]
mod runtime;
fn expected() -> serde_json::Value {
    serde_json::json!({"schema": 1, "targets": {"test_media_format_change": {"sources": ["test/cmocka/test_media_format_change.c"], "libraries": ["OBS::libobs", "OBS::media-playback", "FFmpeg::avcodec", "FFmpeg::avformat", "FFmpeg::avutil", "FFmpeg::swscale", "FFmpeg::swresample", "OBS::native-test-cmocka", {"name": "m", "requires": {"not": "OS_WINDOWS"}}], "requires": "HAVE_MEDIA_PLAYBACK_TEST_OWNER", "watch_inputs": ["shared/media-playback/build/media-playback-cache.pch.h", "shared/media-playback/build/media-playback.cache+.h.c", "shared/media-playback/build/media-playback.core+.h.c", "shared/media-playback/build/media-playback.pch.h", "shared/media-playback/media-playback/cache.c.h", "shared/media-playback/media-playback/cache.h", "shared/media-playback/media-playback/closest-format.h", "shared/media-playback/media-playback/decode.c.h", "shared/media-playback/media-playback/decode.h", "shared/media-playback/media-playback/media-playback.c.h", "shared/media-playback/media-playback/media-playback.h", "shared/media-playback/media-playback/media.c.h", "shared/media-playback/media-playback/media.h", "shared/media-playback/media-playback/network-timeout.c.h", "shared/media-playback/media-playback/network-timeout.h", "shared/media-playback/media-playback/platform-decode-ffmpeg.h", "shared/media-playback/media-playback/platform-media-ffmpeg.h"]}, "test_media_loop_start_time": {"sources": ["test/cmocka/test_media_loop_start_time.c"], "libraries": ["OBS::libobs", "OBS::media-playback", "FFmpeg::avcodec", "FFmpeg::avformat", "FFmpeg::avutil", "FFmpeg::swscale", "FFmpeg::swresample", "OBS::native-test-cmocka", {"name": "m", "requires": {"not": "OS_WINDOWS"}}], "requires": "HAVE_MEDIA_PLAYBACK_TEST_OWNER", "watch_inputs": ["shared/media-playback/build/media-playback-cache.pch.h", "shared/media-playback/build/media-playback.cache+.h.c", "shared/media-playback/build/media-playback.core+.h.c", "shared/media-playback/build/media-playback.pch.h", "shared/media-playback/media-playback/cache.c.h", "shared/media-playback/media-playback/cache.h", "shared/media-playback/media-playback/closest-format.h", "shared/media-playback/media-playback/decode.c.h", "shared/media-playback/media-playback/decode.h", "shared/media-playback/media-playback/media-playback.c.h", "shared/media-playback/media-playback/media-playback.h", "shared/media-playback/media-playback/media.c.h", "shared/media-playback/media-playback/media.h", "shared/media-playback/media-playback/network-timeout.c.h", "shared/media-playback/media-playback/network-timeout.h", "shared/media-playback/media-playback/platform-decode-ffmpeg.h", "shared/media-playback/media-playback/platform-media-ffmpeg.h"]}, "test_media_ts_start_time": {"sources": ["test/cmocka/test_media_ts_start_time.c"], "libraries": ["OBS::libobs", "OBS::media-playback", "FFmpeg::avcodec", "FFmpeg::avformat", "FFmpeg::avutil", "FFmpeg::swscale", "FFmpeg::swresample", "OBS::native-test-cmocka", {"name": "m", "requires": {"not": "OS_WINDOWS"}}], "requires": "HAVE_MEDIA_PLAYBACK_TEST_OWNER", "watch_inputs": ["shared/media-playback/build/media-playback-cache.pch.h", "shared/media-playback/build/media-playback.cache+.h.c", "shared/media-playback/build/media-playback.core+.h.c", "shared/media-playback/build/media-playback.pch.h", "shared/media-playback/media-playback/cache.c.h", "shared/media-playback/media-playback/cache.h", "shared/media-playback/media-playback/closest-format.h", "shared/media-playback/media-playback/decode.c.h", "shared/media-playback/media-playback/decode.h", "shared/media-playback/media-playback/media-playback.c.h", "shared/media-playback/media-playback/media-playback.h", "shared/media-playback/media-playback/media.c.h", "shared/media-playback/media-playback/media.h", "shared/media-playback/media-playback/network-timeout.c.h", "shared/media-playback/media-playback/network-timeout.h", "shared/media-playback/media-playback/platform-decode-ffmpeg.h", "shared/media-playback/media-playback/platform-media-ffmpeg.h"]}, "test_media_seek_keyframe": {"sources": ["test/cmocka/test_media_seek_keyframe.c"], "libraries": ["OBS::libobs", "OBS::media-playback", "FFmpeg::avcodec", "FFmpeg::avformat", "FFmpeg::avutil", "FFmpeg::swscale", "FFmpeg::swresample", "OBS::native-test-cmocka", {"name": "m", "requires": {"not": "OS_WINDOWS"}}], "requires": "HAVE_MEDIA_PLAYBACK_TEST_OWNER", "watch_inputs": ["shared/media-playback/build/media-playback-cache.pch.h", "shared/media-playback/build/media-playback.cache+.h.c", "shared/media-playback/build/media-playback.core+.h.c", "shared/media-playback/build/media-playback.pch.h", "shared/media-playback/media-playback/cache.c.h", "shared/media-playback/media-playback/cache.h", "shared/media-playback/media-playback/closest-format.h", "shared/media-playback/media-playback/decode.c.h", "shared/media-playback/media-playback/decode.h", "shared/media-playback/media-playback/media-playback.c.h", "shared/media-playback/media-playback/media-playback.h", "shared/media-playback/media-playback/media.c.h", "shared/media-playback/media-playback/media.h", "shared/media-playback/media-playback/network-timeout.c.h", "shared/media-playback/media-playback/network-timeout.h", "shared/media-playback/media-playback/platform-decode-ffmpeg.h", "shared/media-playback/media-playback/platform-media-ffmpeg.h"]}, "test_media_alpha_network": {"sources": ["test/cmocka/test_media_alpha_network.c"], "libraries": ["OBS::libobs", "OBS::media-playback", "FFmpeg::avcodec", "FFmpeg::avformat", "FFmpeg::avutil", "FFmpeg::swscale", "FFmpeg::swresample", "OBS::native-test-cmocka", {"name": "m", "requires": {"not": "OS_WINDOWS"}}], "requires": "HAVE_MEDIA_PLAYBACK_TEST_OWNER", "watch_inputs": ["shared/media-playback/build/media-playback-cache.pch.h", "shared/media-playback/build/media-playback.cache+.h.c", "shared/media-playback/build/media-playback.core+.h.c", "shared/media-playback/build/media-playback.pch.h", "shared/media-playback/media-playback/cache.c.h", "shared/media-playback/media-playback/cache.h", "shared/media-playback/media-playback/closest-format.h", "shared/media-playback/media-playback/decode.c.h", "shared/media-playback/media-playback/decode.h", "shared/media-playback/media-playback/media-playback.c.h", "shared/media-playback/media-playback/media-playback.h", "shared/media-playback/media-playback/media.c.h", "shared/media-playback/media-playback/media.h", "shared/media-playback/media-playback/network-timeout.c.h", "shared/media-playback/media-playback/network-timeout.h", "shared/media-playback/media-playback/platform-decode-ffmpeg.h", "shared/media-playback/media-playback/platform-media-ffmpeg.h"]}, "test_media_network_timeout": {"sources": ["test/cmocka/test_media_network_timeout.c"], "libraries": ["OBS::libobs", "OBS::media-playback", "FFmpeg::swscale", "FFmpeg::swresample", "OBS::native-test-cmocka", {"name": "m", "requires": {"not": "OS_WINDOWS"}}], "requires": "HAVE_MEDIA_PLAYBACK_TEST_OWNER", "watch_inputs": ["shared/media-playback/build/media-playback-cache.pch.h", "shared/media-playback/build/media-playback.cache+.h.c", "shared/media-playback/build/media-playback.core+.h.c", "shared/media-playback/build/media-playback.pch.h", "shared/media-playback/media-playback/cache.c.h", "shared/media-playback/media-playback/cache.h", "shared/media-playback/media-playback/closest-format.h", "shared/media-playback/media-playback/decode.c.h", "shared/media-playback/media-playback/decode.h", "shared/media-playback/media-playback/media-playback.c.h", "shared/media-playback/media-playback/media-playback.h", "shared/media-playback/media-playback/media.c.h", "shared/media-playback/media-playback/media.h", "shared/media-playback/media-playback/network-timeout.c.h", "shared/media-playback/media-playback/network-timeout.h", "shared/media-playback/media-playback/platform-decode-ffmpeg.h", "shared/media-playback/media-playback/platform-media-ffmpeg.h"]}}, "tests": {"test_media_format_change": {"target": "test_media_format_change", "timeout_seconds": 120}, "test_media_loop_start_time": {"target": "test_media_loop_start_time", "timeout_seconds": 120}, "test_media_ts_start_time": {"target": "test_media_ts_start_time", "timeout_seconds": 120}, "test_media_seek_keyframe": {"target": "test_media_seek_keyframe", "timeout_seconds": 120}, "test_media_alpha_network": {"target": "test_media_alpha_network", "timeout_seconds": 120}, "test_media_network_timeout": {"target": "test_media_network_timeout", "timeout_seconds": 120}}})
}
#[test]
fn canonical_media_clips_is_registered() {
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
        let text =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(text.contains("test-graph.d"));
        assert!(!text.contains("add_executable("));
        assert!(!text.contains("add_test("));
    }
}
#[test]
fn canonical_media_clips_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.targets.retain(|n, _| CASES.contains(&n.as_str()));
    graph.tests.retain(|n, _| CASES.contains(&n.as_str()));
    assert_eq!(graph.targets.len(), 6);
    for os in [
        "OS_LINUX",
        "OS_FREEBSD",
        "OS_OPENBSD",
        "OS_WINDOWS",
        "OS_MACOS",
    ] {
        for owner in [false, true] {
            let mut facts: std::collections::BTreeMap<String, bool> = [
                "OS_LINUX",
                "OS_FREEBSD",
                "OS_OPENBSD",
                "OS_WINDOWS",
                "OS_MACOS",
            ]
            .into_iter()
            .map(|name| (name.into(), name == os))
            .collect();
            facts.insert("HAVE_MEDIA_PLAYBACK_TEST_OWNER".into(), owner);
            let selected = graph.select(&facts).unwrap();
            assert_eq!(
                selected.targets.len(),
                if owner { 6 } else { 0 },
                "{os}/{owner}"
            );
            assert_eq!(selected.tests.len(), selected.targets.len());
        }
    }
}

use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 6] = [
    "test_rtmps_nanostream",
    "test_rtmps_restream",
    "test_rist_open_cleanup",
    "test_srt_open_close",
    "test_media_packet_eof_leak",
    "test_curl_accept_encoding",
];
const FACTS: [&str; 5] = [
    "HAVE_RTMPS_TEST_SDK",
    "HAVE_LIBRIST_TEST_SDK",
    "HAVE_SRT_TEST_SDK",
    "HAVE_MEDIA_PACKET_TEST_SDK",
    "HAVE_CURL_TEST_SDK",
];

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/transport_helpers_runtime.rs"]
mod runtime;

fn expected() -> serde_json::Value {
    serde_json::json!({"schema": 1, "targets": {"test_rtmps_nanostream": {"sources": ["test/cmocka/test_rtmps_nanostream.c"], "libraries": ["OBS::libobs", "OBS::file-updater", "jansson::jansson", "OBS::native-test-cmocka", {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}], "requires": "HAVE_RTMPS_TEST_SDK", "includes": ["."], "watch_inputs": ["plugins/rtmp-services/service-specific/rtmps-upgrade.c.h", "plugins/rtmp-services/build/rtmp-services.pch.h"]}, "test_rtmps_restream": {"sources": ["test/cmocka/test_rtmps_restream.c"], "libraries": ["OBS::libobs", "OBS::file-updater", "jansson::jansson", "OBS::native-test-cmocka", {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}], "requires": "HAVE_RTMPS_TEST_SDK", "includes": ["."], "watch_inputs": ["plugins/rtmp-services/service-specific/rtmps-upgrade.c.h", "plugins/rtmp-services/build/rtmp-services.pch.h"]}, "test_rist_open_cleanup": {"sources": ["test/cmocka/test_rist_open_cleanup.c"], "libraries": ["OBS::libobs", "Librist::Librist", "FFmpeg::avformat", "FFmpeg::avutil", "OBS::native-test-cmocka"], "requires": "HAVE_LIBRIST_TEST_SDK", "includes": ["plugins/obs-ffmpeg"], "watch_inputs": ["plugins/obs-ffmpeg/obs-ffmpeg-rist.h"], "header_libraries": ["OBS::ffmpeg-net-host"]}, "test_srt_open_close": {"sources": ["test/cmocka/test_srt_open_close.c"], "libraries": ["OBS::libobs", "FFmpeg::avformat", "FFmpeg::avutil", "Libsrt::Libsrt", "OBS::native-test-cmocka"], "requires": "HAVE_SRT_TEST_SDK", "includes": ["plugins/obs-ffmpeg"], "link_options": ["-Wl,--wrap=srt_startup", "-Wl,--wrap=srt_cleanup"], "watch_inputs": ["plugins/obs-ffmpeg/obs-ffmpeg-srt.h"], "header_libraries": ["OBS::ffmpeg-net-host"]}, "test_media_packet_eof_leak": {"sources": ["test/cmocka/test_media_packet_eof_leak.c", "shared/media-playback/media-playback/decode.c.h", "shared/media-playback/media-playback/network-timeout.c.h"], "libraries": ["OBS::libobs", "FFmpeg::avcodec", "FFmpeg::avdevice", "FFmpeg::avformat", "FFmpeg::avutil", "FFmpeg::swscale", "FFmpeg::swresample", "OBS::native-test-cmocka", {"name": "m", "requires": {"not": "OS_WINDOWS"}}, {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}], "requires": "HAVE_MEDIA_PACKET_TEST_SDK", "includes": ["shared/media-playback", "shared/media-playback/media-playback"], "watch_inputs": ["shared/media-playback/media-playback/media.c", "shared/media-playback/media-playback/media.c.h", "shared/media-playback/build/media-playback.pch.h", "test/cmocka/data/media_packet_eof_leak.mkv"]}, "test_curl_accept_encoding": {"sources": ["test/cmocka/test_curl_accept_encoding.c"], "libraries": ["CURL::libcurl", "OBS::native-test-cmocka"], "requires": "HAVE_CURL_TEST_SDK", "includes": ["."], "watch_inputs": ["shared/file-updater/file-updater/curl-accept-encoding.h"]}}, "tests": {"test_rtmps_nanostream": {"target": "test_rtmps_nanostream", "requires": "HAVE_RTMPS_TEST_SDK", "timeout_seconds": 10}, "test_rtmps_restream": {"target": "test_rtmps_restream", "requires": "HAVE_RTMPS_TEST_SDK", "timeout_seconds": 10}, "test_rist_open_cleanup": {"target": "test_rist_open_cleanup", "requires": "HAVE_LIBRIST_TEST_SDK"}, "test_srt_open_close": {"target": "test_srt_open_close", "requires": "HAVE_SRT_TEST_SDK", "timeout_seconds": 60}, "test_media_packet_eof_leak": {"target": "test_media_packet_eof_leak", "requires": "HAVE_MEDIA_PACKET_TEST_SDK", "working_directory": "${build}", "timeout_seconds": 20, "command": ["${target}", "${source}/test/cmocka/data/media_packet_eof_leak.mkv"]}, "test_curl_accept_encoding": {"target": "test_curl_accept_encoding", "requires": "HAVE_CURL_TEST_SDK"}}})
}

/// Refs #1390: retain original native sources, dependencies and test commands.
#[test]
fn canonical_transport_helpers_are_registered() {
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
fn canonical_transport_helpers_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph
        .targets
        .retain(|name, _| CASES.contains(&name.as_str()));
    graph.tests.retain(|name, _| CASES.contains(&name.as_str()));
    assert_eq!(graph.targets.len(), CASES.len());
    for os in ["OS_LINUX", "OS_MACOS", "OS_FREEBSD", "OS_WINDOWS"] {
        for mask in 0..32 {
            let mut facts =
                std::collections::BTreeMap::from([("OS_WINDOWS".into(), os == "OS_WINDOWS")]);
            for (index, name) in FACTS.iter().enumerate() {
                facts.insert((*name).into(), mask & (1 << index) != 0);
            }
            let selected = graph.select(&facts).unwrap();
            let count = (0..5)
                .map(|i| {
                    if mask & (1 << i) != 0 {
                        if i == 0 { 2 } else { 1 }
                    } else {
                        0
                    }
                })
                .sum::<usize>();
            assert_eq!(selected.targets.len(), count, "{os}/{mask}");
            assert_eq!(selected.tests.len(), count, "{os}/{mask}");
        }
    }
}

use super::*;

/// #1390: retain the unchanged real-core packet batching contract.
#[test]
fn canonical_ffmpeg_packet_write_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let name = "test_ffmpeg_mux_write_packet";
    let target = raw["targets"]
        .get(name)
        .expect("missing real-libobs packet write test");
    assert_eq!(
        target["sources"],
        serde_json::json!([
            "test/cmocka/test_ffmpeg_mux_write_packet.c",
            "plugins/obs-ffmpeg/obs-ffmpeg-mux.c.h",
            "plugins/obs-ffmpeg/build/obs-ffmpeg.mux-host-posix+.h.c"
        ])
    );
    assert_eq!(
        target["includes"],
        serde_json::json!(["plugins/obs-ffmpeg"])
    );
    assert_eq!(
        target["libraries"],
        serde_json::json!([
            "OBS::libobs",
            "FFmpeg::avformat",
            "FFmpeg::avutil",
            "OBS::native-test-cmocka"
        ])
    );
    assert_eq!(target["requires"], "OS_LINUX");
    assert_eq!(
        target["link_options"],
        serde_json::json!(["-Wl,--wrap=os_process_pipe_write"])
    );
    assert_eq!(
        raw["tests"][name],
        serde_json::json!({
            "target": name, "requires": "OS_LINUX", "timeout_seconds": 60
        })
    );
    let fragment =
        fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    let mut graph = obs_build::validation::graph::Graph::parse(&raw.to_string()).unwrap();
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in [
        "OS_LINUX",
        "OS_MACOS",
        "OS_WINDOWS",
        "OS_FREEBSD",
        "OS_OPENBSD",
    ] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        let count = usize::from(os == "OS_LINUX");
        assert_eq!(selected.targets.len(), count);
        assert_eq!(selected.tests.len(), count);
    }
}

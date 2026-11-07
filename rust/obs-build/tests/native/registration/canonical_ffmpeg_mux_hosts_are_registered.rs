use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 2] = [
    "test_obs_ffmpeg_mux_host_macos",
    "test_obs_ffmpeg_mux_host_posix",
];

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "ffmpeg_mux_hosts_runtime.rs"]
mod runtime;

fn expected_target(name: &str) -> serde_json::Value {
    let host = name.strip_prefix("test_obs_ffmpeg_mux_host_").unwrap();
    let source = format!("plugins/obs-ffmpeg/obs-ffmpeg-mux-host-{host}.c.h");
    serde_json::json!({
        "sources": ["test/cmocka/test_obs_ffmpeg_mux_host.c"],
        "includes": ["."],
        "definitions": [format!("OBS_FFMPEG_MUX_HOST_SOURCE=\"{source}\"")],
        "watch_inputs": [source],
        "libraries": ["OBS::native-test-cmocka"],
        "requires": {"not": "OS_WINDOWS"}
    })
}

/// Refs #1390: preserve the original FFmpeg mux host helpers.
#[test]
fn canonical_ffmpeg_mux_hosts_are_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    for name in CASES {
        assert_eq!(raw["targets"][name], expected_target(name), "{name}");
        assert_eq!(
            raw["tests"][name],
            serde_json::json!({
                "target": name,
                "requires": {"not": "OS_WINDOWS"}
            }),
            "{name}"
        );
    }
    for helper in ["ffmpeg_mux"] {
        let fragment = fs::read_to_string(
            root.join(format!("test/cmocka/tests.d/test_obs_{helper}_host.cmake")),
        )
        .unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
        assert!(fragment.contains("test-graph.json"));
    }
}

#[test]
fn canonical_ffmpeg_mux_hosts_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph
        .targets
        .retain(|name, _| CASES.contains(&name.as_str()));
    graph.tests.retain(|name, _| CASES.contains(&name.as_str()));
    assert_eq!(graph.targets.len(), CASES.len());
    for os in ["OS_LINUX", "OS_MACOS", "OS_FREEBSD", "OS_WINDOWS"] {
        let windows = os == "OS_WINDOWS";
        let selected = graph
            .select(&[(os.into(), true), ("OS_WINDOWS".into(), windows)].into())
            .unwrap();
        let expected = if windows { 0 } else { CASES.len() };
        assert_eq!(selected.targets.len(), expected, "{os}");
        assert_eq!(selected.tests.len(), expected, "{os}");
    }
}

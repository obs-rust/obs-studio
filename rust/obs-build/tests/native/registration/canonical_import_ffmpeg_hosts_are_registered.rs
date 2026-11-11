use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 4] = [
    "test_obs_ffmpeg_host_linux",
    "test_obs_ffmpeg_host_posix",
    "test_obs_scripting_python_import_host_macos",
    "test_obs_scripting_python_import_host_windows",
];

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/import_ffmpeg_hosts_runtime.rs"]
mod runtime;

fn expected_target(name: &str) -> serde_json::Value {
    let (helper, host) = name
        .strip_prefix("test_obs_")
        .unwrap()
        .split_once("_host_")
        .unwrap();
    let (source, macro_name) = if helper == "ffmpeg" {
        (
            format!("plugins/obs-ffmpeg/obs-ffmpeg-host-{host}.c.h"),
            "OBS_FFMPEG_HOST_SOURCE",
        )
    } else {
        (
            format!("shared/obs-scripting/obs-scripting-python-import-host-{host}.c.h"),
            "OBS_PYTHON_IMPORT_HOST_SOURCE",
        )
    };
    let mut definitions = vec![];
    if helper != "ffmpeg" {
        definitions.push(format!("TEST_HOST_{}", host.to_uppercase()));
    }
    definitions.push(format!("{macro_name}=\"{source}\""));
    let libraries = if helper == "ffmpeg" {
        vec!["OBS::native-test-cmocka", "OBS::libobs"]
    } else {
        vec!["OBS::libobs", "OBS::native-test-cmocka"]
    };
    serde_json::json!({
        "sources": [format!("test/cmocka/test_obs_{helper}_host.c")],
        "includes": ["."],
        "definitions": definitions,
        "watch_inputs": [source],
        "libraries": libraries,
        "requires": {"not": "OS_WINDOWS"}
    })
}

/// Refs #1390: preserve the original Python import and FFmpeg host helpers.
#[test]
fn canonical_import_ffmpeg_hosts_are_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
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
    for helper in ["ffmpeg", "scripting_python_import"] {
        let fragment = fs::read_to_string(
            root.join(format!("test/cmocka/tests.d/test_obs_{helper}_host.cmake")),
        )
        .unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
        assert!(fragment.contains("test-graph.d"));
    }
}

#[test]
fn canonical_import_ffmpeg_hosts_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
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

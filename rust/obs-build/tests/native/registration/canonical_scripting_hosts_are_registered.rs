use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 6] = [
    "test_obs_scripting_lua_host_macos",
    "test_obs_scripting_lua_host_posix",
    "test_obs_scripting_lua_host_windows",
    "test_obs_scripting_python_host_macos",
    "test_obs_scripting_python_host_posix",
    "test_obs_scripting_python_host_windows",
];

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/scripting_hosts_runtime.rs"]
mod runtime;

fn expected_target(name: &str) -> serde_json::Value {
    let (language, host) = name
        .strip_prefix("test_obs_scripting_")
        .unwrap()
        .split_once("_host_")
        .unwrap();
    let source = format!("shared/obs-scripting/obs-scripting-{language}-host-{host}.c.h");
    let scripts = if language == "lua" {
        "test-scripts".into()
    } else {
        format!("test-python-scripts-{host}")
    };
    serde_json::json!({
        "sources": [format!("test/cmocka/test_obs_scripting_{language}_host.c")],
        "includes": ["."],
        "definitions": [
            format!("TEST_HOST_{}", host.to_uppercase()),
            format!("SCRIPT_DIR=\"{scripts}\""),
            format!("OBS_{}_HOST_SOURCE=\"{source}\"", language.to_uppercase())
        ],
        "watch_inputs": [source],
        "libraries": ["OBS::libobs", "OBS::native-test-cmocka"],
        "requires": {"not": "OS_WINDOWS"}
    })
}

/// Refs #1390: preserve the six original host helpers and their shared lock.
#[test]
fn canonical_scripting_hosts_are_registered() {
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
                "requires": {"not": "OS_WINDOWS"},
                "resource_locks": ["obs-scripting-host-plugins-dir"]
            }),
            "{name}"
        );
    }
    for language in ["lua", "python"] {
        let fragment = fs::read_to_string(root.join(format!(
            "test/cmocka/tests.d/test_obs_scripting_{language}_host.cmake"
        )))
        .unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
        assert!(fragment.contains("test-graph.d"));
    }
}

#[test]
fn canonical_scripting_hosts_guards() {
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

use super::*;

/// #1390: the unchanged POSIX output helpers must reach both native consumers.
#[test]
fn canonical_posix_output_hosts_are_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let names = ["test_obs_ffmpeg_net_host", "test_obs_outputs_host"];
    for name in names {
        assert_posix_output_host_registration(&root, &graph, name);
    }
    graph.targets.retain(|key, _| names.contains(&key.as_str()));
    graph.tests.retain(|key, _| names.contains(&key.as_str()));
    for (os, windows) in [
        ("OS_LINUX", false),
        ("OS_MACOS", false),
        ("OS_FREEBSD", false),
        ("OS_WINDOWS", true),
    ] {
        let selected = graph
            .select(&[(os.into(), true), ("OS_WINDOWS".into(), windows)].into())
            .unwrap();
        for name in names {
            assert_eq!(selected.targets.contains_key(name), !windows, "{os}/{name}");
            assert_eq!(selected.tests.contains_key(name), !windows, "{os}/{name}");
        }
    }
}

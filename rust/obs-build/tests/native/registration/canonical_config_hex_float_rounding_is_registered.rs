use super::*;

/// #3664: retain the unchanged bit-exact and locale configuration cases.
#[test]
fn canonical_config_hex_float_rounding_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.retain_suite("core");
    let name = "test_config_hex_float_rounding";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits hex-float configuration coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_config_hex_float_rounding.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            obs_build::validation::graph::ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("hex-float configuration libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, Some(20));
    assert!(
        !root
            .join("test/cmocka/tests.d/test_config_hex_float_rounding.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}

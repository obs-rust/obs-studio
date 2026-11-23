use super::*;

/// #3679: retain the unchanged public encoder group NULL ownership regression.
#[test]
fn canonical_encoder_group_null_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.retain_suite("core");
    let name = "test_encoder_group_null";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits encoder group NULL compatibility coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_encoder_group_null.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("encoder group NULL libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert_eq!(
        test.optional_prefixes,
        [["xvfb-run", "-a", "-s", "-screen 0 1280x1024x24 -noreset"]]
    );
    assert!(
        !root
            .join("test/cmocka/tests.d/test_encoder_group_null.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
        assert_eq!(
            selected.tests[name].optional_prefixes.is_empty(),
            os != "OS_LINUX"
        );
    }
}

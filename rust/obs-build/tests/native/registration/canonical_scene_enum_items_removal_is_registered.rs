use super::*;

/// #3694: retain all unchanged scene enumeration removal lifetime cases.
#[test]
fn canonical_scene_enum_items_removal_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_scene_enum_items_removal";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits scene enumeration removal lifetime coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(matches!(&target.sources[0], Source::Path(path)
        if path == "test/cmocka/test_scene_enum_items_removal.c"));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("scene enumeration libraries must be unconditional"),
        })
        .collect();
    assert_eq!(
        libraries,
        ["OBS::libobs", "OBS::native-test-cmocka", "Threads::Threads"]
    );
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert_eq!(test.optional_prefixes, [vec!["xvfb-run", "-a"]]);
    assert!(
        !root
            .join("test/cmocka/tests.d/test_scene_enum_items_removal.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(
                &[
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                ]
                .into(),
            )
            .unwrap();
        assert_eq!(selected.targets.contains_key(name), os != "OS_WINDOWS");
        assert_eq!(selected.tests.contains_key(name), os != "OS_WINDOWS");
        if let Some(test) = selected.tests.get(name) {
            assert_eq!(!test.optional_prefixes.is_empty(), os == "OS_LINUX");
        }
    }
}

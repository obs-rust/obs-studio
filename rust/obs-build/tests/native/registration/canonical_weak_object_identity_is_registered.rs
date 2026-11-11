use super::*;

/// #4055: retain actual-library coverage and private-header dependencies.
#[test]
fn canonical_weak_object_identity_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Requirement, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    let name = "test_weak_object_identity";
    let target = &graph.targets[name];
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_weak_object_identity.c")
    );
    assert_eq!(
        format!("{:?}", target.libraries),
        format!(
            "{:?}",
            [
                ConditionalValue::Name("OBS::libobs".into()),
                ConditionalValue::Name("OBS::caption".into()),
                ConditionalValue::Name("Uthash::Uthash".into()),
                ConditionalValue::Name("OBS::native-test-cmocka".into()),
                ConditionalValue::Conditional {
                    name: "OBS::w32-pthreads".into(),
                    requires: Requirement::Fact("OS_WINDOWS".into())
                },
            ]
        )
    );
    assert_eq!(graph.tests[name].target, name);
    assert_eq!(graph.tests[name].command, ["${target}"]);
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(
                &["OS_LINUX", "OS_MACOS", "OS_WINDOWS"]
                    .into_iter()
                    .map(|fact| (fact.into(), fact == os))
                    .collect(),
            )
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}

use super::*;

/// #3931: preserve actual-library coverage and the private-header dependencies.
#[test]
fn canonical_encoder_gpu_scaling_getters_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Requirement, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_encoder_gpu_scaling_getters";
    let target = &graph.targets[name];
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_encoder_gpu_scaling_getters.c")
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
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
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

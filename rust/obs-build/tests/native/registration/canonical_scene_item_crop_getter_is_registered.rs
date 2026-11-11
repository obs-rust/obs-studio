use super::*;

/// #3897: the crop characterization is present once with private-header deps.
#[test]
fn canonical_scene_item_crop_getter_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Requirement, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    let name = "test_scene_item_crop_getter";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    let target = &graph.targets[name];
    assert!(
        matches!(target.sources.as_slice(), [Source::Path(path)] if path == "test/cmocka/test_scene_item_crop_getter.c")
    );
    assert_eq!(target.includes, ["libobs", "deps/libcaption"]);
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            ConditionalValue::Conditional { name, .. } => name.as_str(),
        })
        .collect();
    assert_eq!(
        libraries,
        [
            "OBS::libobs",
            "OBS::caption",
            "Uthash::Uthash",
            "OBS::native-test-cmocka",
            "OBS::w32-pthreads"
        ]
    );
    assert!(
        matches!(&target.libraries[4], ConditionalValue::Conditional { name, requires: Requirement::Fact(fact) }
        if name == "OBS::w32-pthreads" && fact == "OS_WINDOWS")
    );
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.suite, "core");
    assert_eq!(test.timeout_seconds, Some(30));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(
                &[
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_MACOS".into(), os == "OS_MACOS"),
                    ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                ]
                .into(),
            )
            .unwrap();
        assert_eq!(selected.targets.len(), 1);
        assert_eq!(selected.tests.len(), 1);
    }
}

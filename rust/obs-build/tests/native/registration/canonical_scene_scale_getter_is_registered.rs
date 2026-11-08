use super::*;

#[test]
fn canonical_scene_scale_getter_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Requirement, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_scene_item_scale_getter";
    let target = &graph.targets[name];
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_scene_item_scale_getter.c"));
    assert_eq!(target.includes, ["libobs", "deps/libcaption"]);
    let [
        ConditionalValue::Name(core),
        ConditionalValue::Name(caption),
        ConditionalValue::Name(uthash),
        ConditionalValue::Name(cmocka),
        ConditionalValue::Conditional {
            name: pthreads,
            requires,
        },
    ] = target.libraries.as_slice()
    else {
        panic!("private scene header dependencies must be retained");
    };
    assert_eq!(
        [
            core.as_str(),
            caption.as_str(),
            uthash.as_str(),
            cmocka.as_str()
        ],
        [
            "OBS::libobs",
            "OBS::caption",
            "Uthash::Uthash",
            "OBS::native-test-cmocka"
        ]
    );
    assert_eq!(pthreads, "OBS::w32-pthreads");
    assert!(matches!(requires, Requirement::Fact(fact) if fact == "OS_WINDOWS"));
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.suite, "core");
    assert_eq!(test.timeout_seconds, Some(30));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert_eq!(selected.targets.len(), 1);
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}

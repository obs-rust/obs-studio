use super::*;

/// #3702: retain all unchanged bounds placement alignment, crop and flip cases.
#[test]
fn canonical_scene_bounds_place_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Requirement, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_scene_bounds_place";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits scene bounds placement compatibility coverage");
    let sources: Vec<_> = target
        .sources
        .iter()
        .map(|source| match source {
            Source::Path(path) => path.as_str(),
            _ => panic!("bounds placement sources must be unconditional"),
        })
        .collect();
    assert_eq!(
        sources,
        [
            "test/cmocka/test_scene_bounds_place.c",
            "libobs/obs-scene-bounds-place.c.h",
            "libobs/obs-scene-bounds.c.h"
        ]
    );
    assert!(target.includes.is_empty());
    assert_eq!(target.watch_inputs, ["libobs/obs-scene-bounds.h"]);
    assert_eq!(target.sanitizers, None);
    let [
        ConditionalValue::Name(core),
        ConditionalValue::Name(cmocka),
        ConditionalValue::Conditional {
            name: library,
            requires,
        },
    ] = target.libraries.as_slice()
    else {
        panic!("bounds placement needs core, cmocka and conditional libm");
    };
    assert_eq!(core, "OBS::libobs");
    assert_eq!(cmocka, "OBS::native-test-cmocka");
    assert_eq!(library, "m");
    assert!(matches!(requires, Requirement::Not { not }
        if matches!(not.as_ref(), Requirement::Fact(fact) if fact == "MSVC")));
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_scene_bounds_place.cmake")).unwrap();
    assert!(!fragment.contains("add_executable(\n  test_scene_bounds_place\n"));
    assert!(!fragment.contains("add_executable("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}

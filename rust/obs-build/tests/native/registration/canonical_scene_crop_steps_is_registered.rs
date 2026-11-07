use super::*;

/// #3699: retain unchanged crop/size saturation cases and overflow sanitizers.
#[test]
fn canonical_scene_crop_steps_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Requirement, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_scene_crop_steps";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits scene crop arithmetic compatibility coverage");
    let sources: Vec<_> = target
        .sources
        .iter()
        .map(|source| match source {
            Source::Path(path) => path.as_str(),
            _ => panic!("scene crop sources must be unconditional"),
        })
        .collect();
    assert_eq!(
        sources,
        [
            "test/cmocka/test_scene_crop_steps.c",
            "libobs/obs-scene-crop.c.h"
        ]
    );
    assert_eq!(target.includes, ["libobs", "deps/libcaption"]);
    assert_eq!(
        target.sanitizers.as_deref(),
        Some("undefined,float-cast-overflow")
    );
    assert_eq!(target.libraries.len(), 5);
    for (library, expected) in target.libraries.iter().zip([
        "OBS::libobs",
        "OBS::caption",
        "Uthash::Uthash",
        "OBS::native-test-cmocka",
    ]) {
        assert!(matches!(library, ConditionalValue::Name(name) if name == expected));
    }
    let ConditionalValue::Conditional {
        name: library,
        requires,
    } = &target.libraries[4]
    else {
        panic!("libm must retain its non-MSVC guard");
    };
    assert_eq!(library, "m");
    assert!(matches!(requires, Requirement::Not { not }
        if matches!(not.as_ref(), Requirement::Fact(fact) if fact == "MSVC")));
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert!(
        !root
            .join("test/cmocka/tests.d/test_scene_crop_steps.cmake")
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

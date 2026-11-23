use super::*;

/// #3705: retain the full scene collection crop-load integration suite.
#[test]
fn canonical_scene_item_crop_load_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.retain_suite("graphics");
    let name = "test_scene_item_crop_load";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits scene collection crop-load compatibility coverage");
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_scene_item_crop_load.c"));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("crop-load linkage must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    assert_eq!(
        target
            .artifact_definitions
            .get("GRAPHICS_MODULE_PATH")
            .map(String::as_str),
        Some("OBS::libobs-opengl")
    );
    assert_eq!(target.watch_inputs, ["libobs/data"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.working_directory.as_deref(), Some("${runtime}"));
    assert_eq!(test.timeout_seconds, Some(120));
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_scene_item_crop_load.cmake"))
            .unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for (os, graphics, expected) in [
        ("OS_LINUX", true, true),
        ("OS_LINUX", false, false),
        ("OS_MACOS", true, false),
        ("OS_WINDOWS", true, false),
    ] {
        let selected = graph
            .select(
                &[
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_MACOS".into(), os == "OS_MACOS"),
                    ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                    ("HAVE_OBS_OPENGL".into(), graphics),
                ]
                .into(),
            )
            .unwrap();
        assert_eq!(
            selected.targets.contains_key(name),
            expected,
            "{os}, OpenGL={graphics}"
        );
        assert_eq!(
            selected.tests.contains_key(name),
            expected,
            "{os}, OpenGL={graphics}"
        );
    }
}

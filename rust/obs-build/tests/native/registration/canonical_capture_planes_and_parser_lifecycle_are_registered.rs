use super::*;

/// #1390: run portable production helpers with the original real-core linkage.
#[test]
fn canonical_capture_planes_and_parser_lifecycle_are_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    for (name, source, includes, requires, libraries) in [
        (
            "test_avcapture_audio_planes",
            "plugins/mac-avcapture/avcapture-audio-planes.c.h",
            vec!["plugins/mac-avcapture"],
            serde_json::json!("HAVE_LIBOBS"),
            vec!["OBS::libobs", "OBS::native-test-cmocka"],
        ),
        (
            "test_effect_parser_lifecycle",
            "libobs/graphics/effect-parser-lifecycle.c.h",
            vec!["deps/libcaption", "libobs"],
            serde_json::json!(true),
            vec![
                "OBS::libobs",
                "OBS::caption",
                "Uthash::Uthash",
                "OBS::native-test-cmocka",
            ],
        ),
    ] {
        let target = raw["targets"]
            .get(name)
            .expect("missing portable real-libobs helper");
        assert_eq!(
            target["sources"],
            serde_json::json!([format!("test/cmocka/{name}.c"), source])
        );
        assert_eq!(target["includes"], serde_json::json!(includes));
        let mut libraries = serde_json::json!(libraries);
        if name == "test_effect_parser_lifecycle" {
            libraries.as_array_mut().unwrap().push(serde_json::json!({
                "name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"
            }));
        }
        assert_eq!(target["libraries"], libraries);
        assert_eq!(target["requires"], requires);
        assert_eq!(
            raw["tests"][name],
            serde_json::json!({"target": name, "requires": requires})
        );
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains(&format!("add_test({name} ")));
        assert!(!fragment.contains(&format!("add_executable({name} ")));
    }
    assert_eq!(
        raw["targets"]["test_effect_parser_lifecycle"]["sanitizers"],
        "address,undefined"
    );
    let mut graph = obs_build::validation::graph::Graph::parse(&raw.to_string()).unwrap();
    let names = [
        "test_avcapture_audio_planes",
        "test_effect_parser_lifecycle",
    ];
    graph
        .targets
        .retain(|name, _| names.contains(&name.as_str()));
    graph.tests.retain(|name, _| names.contains(&name.as_str()));
    for os in [
        "OS_LINUX",
        "OS_MACOS",
        "OS_WINDOWS",
        "OS_FREEBSD",
        "OS_OPENBSD",
    ] {
        for have_core in [false, true] {
            let selected = graph
                .select(&[(os.into(), true), ("HAVE_LIBOBS".into(), have_core)].into())
                .unwrap();
            let count = if have_core { 2 } else { 1 };
            assert_eq!(selected.targets.len(), count);
            assert_eq!(selected.tests.len(), count);
        }
    }
}

use super::*;

/// #3716: both native consumers must retain video-output compatibility coverage.
#[test]
fn canonical_video_output_compatibility_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    let cases = [
        ("test_video_output_formats", 180),
        ("test_video_reset_fps", 60),
    ];
    for (name, timeout) in cases {
        let target = graph.targets.get(name).unwrap_or_else(|| {
            panic!("canonical graph omits video-output compatibility coverage: {name}")
        });
        assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
            if path == &format!("test/cmocka/{name}.c")));
        let libraries: Vec<_> = target
            .libraries
            .iter()
            .map(|library| match library {
                ConditionalValue::Name(name) => name.as_str(),
                _ => panic!("video-output linkage must be unconditional"),
            })
            .collect();
        assert_eq!(
            libraries,
            ["OBS::libobs", "OBS::native-test-cmocka"],
            "{name}"
        );
        assert!(target.header_libraries.is_empty());
        assert_eq!(
            target.artifact_definitions["GRAPHICS_MODULE_PATH"],
            "OBS::libobs-opengl"
        );
        assert_eq!(target.watch_inputs, ["libobs/data"]);
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.suite, "graphics");
        assert_eq!(test.working_directory.as_deref(), Some("${runtime}"));
        assert_eq!(test.timeout_seconds, Some(timeout));
        assert_eq!(test.skip_return_code, None);
        assert_eq!(
            test.optional_prefixes,
            [vec!["xvfb-run".to_string(), "-a".to_string()]]
        );
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
    graph
        .targets
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    graph
        .tests
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for opengl in [true, false] {
            let selected = graph
                .select(
                    &[
                        ("OS_LINUX".into(), os == "OS_LINUX"),
                        ("OS_MACOS".into(), os == "OS_MACOS"),
                        ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                        ("HAVE_OBS_OPENGL".into(), opengl),
                    ]
                    .into(),
                )
                .unwrap();
            for (name, _) in cases {
                assert_eq!(
                    selected.targets.contains_key(name),
                    os == "OS_LINUX" && opengl,
                    "{os}/{opengl}/{name}"
                );
                assert_eq!(
                    selected.tests.contains_key(name),
                    os == "OS_LINUX" && opengl,
                    "{os}/{opengl}/{name}"
                );
            }
        }
    }
}

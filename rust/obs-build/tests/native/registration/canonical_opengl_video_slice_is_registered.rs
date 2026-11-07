use super::*;

/// #1390: preserve real OpenGL video compatibility coverage in both consumers.
#[test]
fn canonical_opengl_video_slice_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let cases = [("test_visibility_transition", None)];
    for (name, timeout) in cases {
        let target = graph.targets.get(name).unwrap_or_else(|| {
            panic!("canonical graph omits transition compatibility coverage: {name}")
        });
        let mut sources = vec![format!("test/cmocka/{name}.c")];
        if name == "test_visibility_transition" {
            sources.push("frontend/utility/visibility-transition.c.h".into());
            assert_eq!(target.includes, ["frontend/utility"]);
        } else {
            assert!(target.includes.is_empty());
        }
        let actual: Vec<_> = target
            .sources
            .iter()
            .map(|source| match source {
                Source::Path(path) => path.clone(),
                _ => panic!("conditional source"),
            })
            .collect();
        assert_eq!(actual, sources);
        let libraries: Vec<_> = target
            .libraries
            .iter()
            .map(|library| match library {
                ConditionalValue::Name(name) => name.as_str(),
                _ => panic!("transition linkage must be unconditional"),
            })
            .collect();
        let expected = vec!["OBS::libobs", "OBS::native-test-cmocka"];
        assert!(target.header_libraries.is_empty());
        assert_eq!(libraries, expected, "{name}");
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
        assert_eq!(test.timeout_seconds, timeout);
        assert_eq!(test.skip_return_code, None);
        assert!(test.optional_prefixes.is_empty());
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

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "opengl_video_slice_runtime.rs"]
mod runtime;

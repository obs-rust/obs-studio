use super::*;

/// #1390: the portable virtual-microphone helpers need both native consumers.
#[test]
fn canonical_vmic_helpers_are_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let names = ["test_mac_vmic_clock", "test_mac_vmic_install"];
    for name in names {
        let target = graph
            .targets
            .get(name)
            .unwrap_or_else(|| panic!("canonical graph omits {name}"));
        assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
            if path == &format!("test/cmocka/{name}.c")));
        assert!(
            matches!(target.libraries.as_slice(), [ConditionalValue::Name(library)]
            if library == "OBS::native-test-cmocka")
        );
        assert_eq!(target.includes, ["plugins/mac-virtualcam/src/common"]);
        assert_eq!(target.watch_inputs, ["plugins/mac-virtualcam/src/common"]);
        assert!(target.header_libraries.is_empty());
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.suite, "core");
        assert_eq!(test.timeout_seconds, None);
        assert_eq!(test.skip_return_code, None);
        assert!(test.optional_prefixes.is_empty());
        assert_eq!(test.qt_runtime, None);
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
    graph.targets.retain(|key, _| names.contains(&key.as_str()));
    graph.tests.retain(|key, _| names.contains(&key.as_str()));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        for name in names {
            assert!(selected.targets.contains_key(name), "{os}/{name}");
            assert!(selected.tests.contains_key(name), "{os}/{name}");
        }
    }
}

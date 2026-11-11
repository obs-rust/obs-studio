use super::*;

/// #1390: standalone helper tests preserve platform and header requirements.
#[test]
fn canonical_nvenc_host_and_glyph_tests_are_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    let names = [
        "test_nvenc_override",
        "test_hotkey_numpad_host",
        "test_libobs_reference_hosts",
        "test_glyph_atlas",
    ];
    for name in names {
        let target = graph
            .targets
            .get(name)
            .unwrap_or_else(|| panic!("canonical graph omits {name}"));
        assert!(
            matches!(target.libraries.as_slice(), [ConditionalValue::Name(library)] if library == "OBS::native-test-cmocka")
        );
        assert_eq!(graph.tests[name].target, name);
        assert_eq!(graph.tests[name].command, ["${target}"]);
        assert_eq!(graph.tests[name].timeout_seconds, None);
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
    let glyph = &graph.targets["test_glyph_atlas"];
    assert_eq!(glyph.header_packages, ["freetype2"]);
    assert_eq!(glyph.header_libraries, ["OBS::libobs", "SIMDe::SIMDe"]);
    assert_eq!(glyph.includes, ["libobs", "plugins/text-freetype2"]);
    graph
        .targets
        .retain(|name, _| names.contains(&name.as_str()));
    graph.tests.retain(|name, _| names.contains(&name.as_str()));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let facts = ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"]
            .map(|key| (key.into(), key == os))
            .into();
        let selected = graph.select(&facts).unwrap();
        for name in names {
            let expected = match name {
                "test_glyph_atlas" => os == "OS_LINUX",
                "test_hotkey_numpad_host" => os != "OS_MACOS",
                "test_libobs_reference_hosts" => os != "OS_WINDOWS",
                _ => true,
            };
            assert_eq!(selected.targets.contains_key(name), expected, "{os}/{name}");
            assert_eq!(selected.tests.contains_key(name), expected, "{os}/{name}");
        }
    }
}

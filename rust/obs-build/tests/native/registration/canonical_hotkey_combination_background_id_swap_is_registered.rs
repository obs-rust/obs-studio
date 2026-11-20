use super::*;

#[test]
fn canonical_hotkey_combination_background_id_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_hotkey_combination_background_id";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_hotkey_combination_background_id.c")
        );
        assert_eq!(selected.tests[name].target, name);
    }
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/hotkey-combination-background-id.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-hotkey-combination-background-id+.h.c"));
    let group = fs::read_to_string(
        root.join("libobs/build/libobs.ported-hotkey-combination-background-id+.h.c"),
    )
    .unwrap();
    // The C reference gives the original its static lock pair through the seam.
    let lock = "#include \"libobs/obs-hotkey-combination-background-id-lock.c.h\"";
    let original = "#include \"libobs/obs-hotkey-combination-background-id.c.h\"";
    assert!(group.find(lock).unwrap() < group.find(original).unwrap());
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core+.h.c")).unwrap();
    assert!(!hybrid.contains(original));
    // The C-owned mutex, callback slot and dstr_free stay in the production group.
    assert!(hybrid.contains("#include \"libobs/obs-hotkey.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-hotkey-combination-background-id-fields.c.h\""));
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-hotkey-combination_background_id.txt"),
    )
    .unwrap();
    assert_eq!(exports.trim(), "obs_hotkey_combination_get_background_id");
}

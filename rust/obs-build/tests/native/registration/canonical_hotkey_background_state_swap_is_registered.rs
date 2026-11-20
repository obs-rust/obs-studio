use super::*;

const ORIGINAL: &str = "libobs/obs-hotkey-background-state.c.h";

/// The unchanged characterization keeps testing the original; its external
/// twin runs the same assertions on the Rust shim through the real adapters.
#[test]
fn canonical_hotkey_background_state_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let c_test = "test_hotkey_background_state";
    let rust_test = "test_hotkey_background_state_rust";
    graph
        .targets
        .retain(|key, _| key == c_test || key == rust_test);
    graph
        .tests
        .retain(|key, _| key == c_test || key == rust_test);
    let test_source = "test/cmocka/test_hotkey_background_state.c";
    let record = "test/cmocka/hotkey-background-state-record-support.c";
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for available in [false, true] {
            let facts = [
                (os.into(), true),
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert!(matches!(selected.targets[c_test].sources.as_slice(),
                [Source::Path(a)] if a == test_source));
            assert!(selected.targets[c_test].definitions.is_empty());
            assert_eq!(selected.tests[c_test].target, c_test);
            assert_eq!(selected.targets.contains_key(rust_test), available);
            if !available {
                continue;
            }
            let target = &selected.targets[rust_test];
            assert!(matches!(target.sources.as_slice(),
                [Source::Path(a), Source::Path(b)] if a == test_source && b == record));
            assert_eq!(target.unity_groups["background_state"], [test_source, record]);
            assert_eq!(target.definitions, ["HOTKEY_BACKGROUND_STATE_EXTERNAL"]);
            assert_eq!(selected.tests[rust_test].target, rust_test);
        }
    }
    files_are_wired(&root, test_source, record);
}

/// The record support, the ported group, the hybrid group and the export.
fn files_are_wired(root: &Path, test_source: &str, record: &str) {
    let fields = "#include \"libobs/obs-hotkey-background-state-fields.c.h\"";
    let record_text = fs::read_to_string(root.join(record)).unwrap();
    assert!(record_text.contains("#define obs record_core()"));
    assert!(record_text.contains(fields));
    let test_text = fs::read_to_string(root.join(test_source)).unwrap();
    assert!(test_text.contains("#ifndef HOTKEY_BACKGROUND_STATE_EXTERNAL"));
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/hotkey-background-state.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-hotkey-background-state+.h.c"));
    let group = fs::read_to_string(
        root.join("libobs/build/libobs.ported-hotkey-background-state+.h.c"),
    )
    .unwrap();
    // The C reference gives the original its static lock pair through the seam.
    let lock = "#include \"libobs/obs-hotkey-background-state-lock.c.h\"";
    let original = format!("#include \"{ORIGINAL}\"");
    assert!(group.find(lock).unwrap() < group.find(&original).unwrap());
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core+.h.c")).unwrap();
    assert!(!hybrid.contains(&original));
    // The C-owned mutex and fields stay in the production group, ahead of
    // the static lock pair the adapters forward-declare.
    let owner = "#include \"libobs/obs-hotkey.c.h\"";
    assert!(hybrid.find(fields).unwrap() < hybrid.find(owner).unwrap());
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-hotkey-background_state.txt"),
    )
    .unwrap();
    assert_eq!(exports.trim(), "obs_hotkeys_get_background_state");
}

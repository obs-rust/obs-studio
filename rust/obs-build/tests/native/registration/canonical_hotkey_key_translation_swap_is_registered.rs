use super::*;

#[test]
fn canonical_hotkey_key_translation_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let c_test = "test_hotkey_key_translation";
    let rust_test = "test_hotkey_key_translation_rust";
    graph
        .targets
        .retain(|key, _| key == c_test || key == rust_test);
    graph
        .tests
        .retain(|key, _| key == c_test || key == rust_test);
    let test_source = "test/cmocka/test_hotkey_key_translation.c";
    let record = "test/cmocka/hotkey-key-translation-record-support.c";
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for available in [false, true] {
            let facts = [
                (os.into(), true),
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            // The unchanged characterization keeps testing the original C source.
            assert!(
                matches!(selected.targets[c_test].sources.as_slice(), [Source::Path(path)]
                if path == test_source)
            );
            assert_eq!(selected.tests[c_test].target, c_test);
            assert_eq!(selected.targets.contains_key(rust_test), available);
            if !available {
                continue;
            }
            // Its external seam reaches the Rust symbols through the real
            // adapters, bound to the test's own core in one unity group.
            let target = &selected.targets[rust_test];
            assert!(matches!(target.sources.as_slice(),
                [Source::Path(a), Source::Path(b)] if a == test_source && b == record));
            assert_eq!(
                target.unity_groups["key_translation"],
                [test_source, record]
            );
            assert_eq!(target.definitions, ["HOTKEY_KEY_TRANSLATION_EXTERNAL"]);
            assert_eq!(selected.tests[rust_test].target, rust_test);
        }
    }
    let record_text = fs::read_to_string(root.join(record)).unwrap();
    let fields = "#include \"libobs/obs-hotkey-key-translation-fields.c.h\"";
    assert!(record_text.contains("#define obs record_core()"));
    assert!(record_text.contains(fields));
    let test_text = fs::read_to_string(root.join(test_source)).unwrap();
    assert!(test_text.contains("#ifndef HOTKEY_KEY_TRANSLATION_EXTERNAL"));
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/hotkey-key-translation.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-hotkey-key-translation+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-hotkey-key-translation+.h.c"))
            .unwrap();
    let original = "#include \"libobs/obs-hotkey-key-translation.c.h\"";
    assert!(group.contains(original));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core+.h.c")).unwrap();
    assert!(!hybrid.contains(original));
    // The C-owned name array and its allocator stay in the production group.
    assert!(hybrid.contains(fields));
    // Standalone Rust-static consumers get the same real adapters.
    let support =
        fs::read_to_string(root.join("test/cmocka/build/rust-static-support.json")).unwrap();
    assert!(support.contains("\"test/cmocka/hotkey-key-translation-link-support.c\""));
    let link =
        fs::read_to_string(root.join("test/cmocka/hotkey-key-translation-link-support.c")).unwrap();
    assert!(link.contains(fields));
    let hidden =
        fs::read_to_string(root.join("libobs/cmake/rust-hidden.d/obs-hotkey-key_translation.txt"))
            .unwrap();
    let names: Vec<_> = hidden
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .collect();
    assert_eq!(
        names,
        [
            "obs_get_hotkey_translation",
            "obs_hotkeys_store_key_translation"
        ]
    );
}

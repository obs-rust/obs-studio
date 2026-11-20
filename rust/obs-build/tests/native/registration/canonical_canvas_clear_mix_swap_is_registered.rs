use super::*;

#[test]
fn canonical_canvas_clear_mix_swap_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let c_test = "test_canvas_clear_mix";
    let rust_test = "test_canvas_clear_mix_rust";
    graph
        .targets
        .retain(|key, _| key == c_test || key == rust_test);
    graph
        .tests
        .retain(|key, _| key == c_test || key == rust_test);
    let test_source = "test/cmocka/test_canvas_clear_mix.c";
    let null_probe = "test/cmocka/canvas-clear-mix-null-host-linux.c";
    let record = "test/cmocka/canvas-clear-mix-record-support.c";
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for available in [false, true] {
            let facts = [
                (os.into(), true),
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            // The unchanged characterization keeps testing the original C
            // source, with its Linux NULL-canvas probe.
            assert!(matches!(selected.targets[c_test].sources.as_slice(),
                [Source::Path(a), Source::Conditional { path: b, .. }]
                if a == test_source && b == null_probe));
            assert!(selected.targets[c_test].definitions.is_empty());
            assert_eq!(selected.tests[c_test].target, c_test);
            assert_eq!(selected.targets.contains_key(rust_test), available);
            if !available {
                continue;
            }
            // Its external seam reaches the Rust shim through the real
            // adapters, bound to the test's own core and recording teardown
            // in one unity group.
            let target = &selected.targets[rust_test];
            assert!(matches!(target.sources.as_slice(),
                [Source::Path(a), Source::Path(b)] if a == test_source && b == record));
            assert_eq!(target.unity_groups["clear_mix"], [test_source, record]);
            assert_eq!(target.definitions, ["CANVAS_CLEAR_MIX_EXTERNAL"]);
            // The same Linux-only NULL probe, compiled into its group.
            assert!(matches!(target.compile_options.as_slice(),
                [ConditionalValue::Conditional { name, requires }]
                if name == "-DCANVAS_CLEAR_MIX_NULL_PROBE"
                    && format!("{requires:?}") == "Fact(\"OS_LINUX\")"));
            assert_eq!(selected.tests[rust_test].target, rust_test);
        }
    }
    files_are_wired(&root, test_source, record, null_probe);
}

/// The record support, the ported group, the hybrid group and the hidden list.
fn files_are_wired(root: &Path, test_source: &str, record: &str, null_probe: &str) {
    let fields = "#include \"libobs/obs-canvas-clear-mix-fields.c.h\"";
    let record_text = fs::read_to_string(root.join(record)).unwrap();
    assert!(record_text.contains("#define obs current_core"));
    assert!(record_text.contains("#define obs_free_video_mix record_free"));
    assert!(record_text.contains("#define pthread_mutex_lock record_lock"));
    assert!(record_text.contains("#define pthread_mutex_unlock record_unlock"));
    assert!(record_text.contains(fields));
    assert!(record_text.contains(&format!("#include \"{null_probe}\"")));
    let test_text = fs::read_to_string(root.join(test_source)).unwrap();
    assert!(test_text.contains("#ifndef CANVAS_CLEAR_MIX_EXTERNAL"));
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/canvas-clear-mix.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-canvas-clear-mix+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-canvas-clear-mix+.h.c")).unwrap();
    let original = "#include \"libobs/obs-canvas-clear-mix.c.h\"";
    assert!(group.contains(original));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains(original));
    // The C-owned slot, mix list, mutex and teardown stay in the core group.
    assert!(hybrid.contains(fields));
    let hidden =
        fs::read_to_string(root.join("libobs/cmake/rust-hidden.d/obs-core-canvas_clear_mix.txt"))
            .unwrap();
    let names: Vec<_> = hidden
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .collect();
    assert_eq!(names, ["obs_canvas_clear_mix"]);
}

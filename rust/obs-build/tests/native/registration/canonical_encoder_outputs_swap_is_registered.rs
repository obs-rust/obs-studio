use super::*;

const ORIGINAL: &str = "libobs/obs-encoder-outputs.c.h";

/// No other target compiles the original: only the unchanged
/// characterization keeps it, so no standalone Rust-static consumer needs a
/// shared copy of the adapters.
#[test]
fn canonical_encoder_outputs_original_has_one_consumer() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("test/cmocka/build/test-graph.json")).unwrap(),
    )
    .unwrap();
    let consumers: Vec<_> = graph["targets"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, target)| {
            target["sources"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(ORIGINAL))
                || target["unity_groups"].as_object().is_some_and(|groups| {
                    groups.values().any(|group| {
                        group
                            .as_array()
                            .unwrap()
                            .contains(&serde_json::json!(ORIGINAL))
                    })
                })
        })
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(consumers, ["test_encoder_outputs"]);
}

/// The unchanged characterization keeps testing the original; its twin runs
/// the same assertions on the Rust shims through the real adapters.
#[test]
fn canonical_encoder_outputs_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let c_test = "test_encoder_outputs";
    let rust_test = "test_encoder_outputs_rust";
    graph
        .targets
        .retain(|key, _| key == c_test || key == rust_test);
    graph
        .tests
        .retain(|key, _| key == c_test || key == rust_test);
    let test_source = "test/cmocka/test_encoder_outputs.c";
    let record = "test/cmocka/encoder-outputs-record-support.c";
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for available in [false, true] {
            let facts = [
                (os.into(), true),
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            // The characterization compiles the original with its recording
            // lock definitions.
            assert!(matches!(selected.targets[c_test].sources.as_slice(),
                [Source::Path(a), Source::Path(b)] if a == test_source && b == ORIGINAL));
            assert_eq!(
                selected.targets[c_test].definitions,
                [
                    "pthread_mutex_lock=encoder_outputs_lock",
                    "pthread_mutex_unlock=encoder_outputs_unlock"
                ]
            );
            assert_eq!(selected.tests[c_test].target, c_test);
            assert_eq!(selected.targets.contains_key(rust_test), available);
            if !available {
                continue;
            }
            // The twin drops the original and links the Rust shims; the real
            // adapters are bound to the test's recorders in one unity group.
            let target = &selected.targets[rust_test];
            assert!(matches!(target.sources.as_slice(),
                [Source::Path(a), Source::Path(b)] if a == test_source && b == record));
            assert_eq!(
                target.unity_groups["encoder_outputs"],
                [test_source, record]
            );
            assert!(target.definitions.is_empty());
            assert_eq!(selected.tests[rust_test].target, rust_test);
        }
    }
    files_are_wired(&root, record);
}

/// The record support, the ported group, the hybrid group and the hidden list.
fn files_are_wired(root: &Path, record: &str) {
    let fields = "#include \"libobs/obs-encoder-outputs-fields.c.h\"";
    let record_text = fs::read_to_string(root.join(record)).unwrap();
    assert!(record_text.contains("#define pthread_mutex_lock encoder_outputs_lock"));
    assert!(record_text.contains("#define pthread_mutex_unlock encoder_outputs_unlock"));
    assert!(record_text.contains(fields));
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/encoder-outputs.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-encoder-outputs+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-encoder-outputs+.h.c")).unwrap();
    let original = format!("#include \"{ORIGINAL}\"");
    assert!(group.contains(&original));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-output+.h.c")).unwrap();
    assert!(!hybrid.contains(&original));
    // The C-owned list, its buffer and its mutex stay in the core group.
    assert!(hybrid.contains(fields));
    let hidden =
        fs::read_to_string(root.join("libobs/cmake/rust-hidden.d/obs-encoder-output_list.txt"))
            .unwrap();
    let names: Vec<_> = hidden
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .collect();
    assert_eq!(
        names,
        ["obs_encoder_add_output", "obs_encoder_remove_output"]
    );
}

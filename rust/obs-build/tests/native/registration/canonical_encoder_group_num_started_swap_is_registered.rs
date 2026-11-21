use super::*;

const ORIGINAL: &str = "libobs/obs-encoder-group-num-started.c.h";
const FIELDS: &str = "libobs/obs-encoder-group-num-started-fields.c.h";
const SHIM: &str = "obs_encoder_group_num_started";
const ADAPTERS: [&str; 3] = [
    "obs_encoder_group_num_started_lock",
    "obs_encoder_group_num_started_read",
    "obs_encoder_group_num_started_unlock",
];
const DEFINITIONS: [&str; 2] = [
    "pthread_mutex_lock=encoder_group_count_record_lock",
    "pthread_mutex_unlock=encoder_group_count_record_unlock",
];

fn graph_json(root: &Path) -> serde_json::Value {
    serde_json::from_str(
        &fs::read_to_string(root.join("test/cmocka/build/test-graph.json")).unwrap(),
    )
    .unwrap()
}

/// No other target compiles the original, and the only cmocka source that
/// defines the shim's name (output_interleave_fields_stub.c, for the
/// interleaver's C fixture) is linked without the Rust archive, so neither
/// collides with the swap and no standalone Rust-static consumer needs a
/// shared copy of the adapters.
#[test]
fn canonical_encoder_group_num_started_original_has_one_consumer() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph = graph_json(&root);
    let targets = graph["targets"].as_object().unwrap();
    let compiles = |target: &serde_json::Value, source: &str| {
        let source = serde_json::json!(source);
        target["sources"].as_array().unwrap().contains(&source)
            || target["unity_groups"].as_object().is_some_and(|groups| {
                groups
                    .values()
                    .any(|g| g.as_array().unwrap().contains(&source))
            })
    };
    let consumers: Vec<_> = targets
        .iter()
        .filter(|(_, target)| compiles(target, ORIGINAL))
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(consumers, ["test_encoder_group_num_started"]);
    let stub = "test/cmocka/output_interleave_fields_stub.c";
    let stub_text = fs::read_to_string(root.join(stub)).unwrap();
    assert!(stub_text.contains(&format!("uint32_t {SHIM}(struct obs_encoder_group *group)")));
    for (name, target) in targets {
        if compiles(target, stub) {
            let libraries = target["libraries"].to_string();
            assert!(
                !libraries.contains("libobs_rust-static"),
                "{name} defines {SHIM} and links the Rust archive"
            );
        }
    }
}

/// The unchanged characterization keeps testing the original; its twin
/// compiles the same fixture with the same recorder definitions, without the
/// original, so the Rust shim locks and reads through the real adapters and
/// the test's recorders.
#[test]
fn canonical_encoder_group_num_started_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let c_test = "test_encoder_group_num_started";
    let rust_test = "test_encoder_group_num_started_rust";
    graph
        .targets
        .retain(|key, _| key == c_test || key == rust_test);
    graph
        .tests
        .retain(|key, _| key == c_test || key == rust_test);
    let test_source = "test/cmocka/test_encoder_group_num_started.c";
    let record = "test/cmocka/encoder-group-num-started-record-support.c";
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for available in [false, true] {
            let facts = [
                (os.into(), true),
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert!(matches!(selected.targets[c_test].sources.as_slice(),
                [Source::Path(a), Source::Path(b)] if a == test_source && b == ORIGINAL));
            assert_eq!(selected.targets[c_test].definitions, DEFINITIONS);
            assert_eq!(selected.tests[c_test].target, c_test);
            assert_eq!(selected.targets.contains_key(rust_test), available);
            if !available {
                continue;
            }
            let target = &selected.targets[rust_test];
            assert!(matches!(target.sources.as_slice(),
                [Source::Path(a), Source::Path(b)] if a == test_source && b == record));
            assert_eq!(target.definitions, DEFINITIONS);
            assert_eq!(selected.tests[rust_test].target, rust_test);
        }
    }
    files_are_wired(&root, record);
}

/// Darwin binds an executable's calls by two-level namespace: a name
/// libobs.dylib exports would bind there, past the twin's own definitions
/// (#5226). The shim is hidden in both libraries and the adapters are never
/// exported, so the twin can only take the shim from the Rust archive and
/// the adapters (with the test's recorders) from its own objects.
#[test]
fn canonical_encoder_group_num_started_names_stay_private() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let names = |dir: &str| -> Vec<String> {
        let mut names = Vec::new();
        for entry in fs::read_dir(root.join(dir)).unwrap() {
            let text = fs::read_to_string(entry.unwrap().path()).unwrap();
            names.extend(
                text.lines()
                    .map(str::trim)
                    .filter(|line| !line.starts_with('#') && !line.is_empty())
                    .map(str::to_owned),
            );
        }
        names
    };
    let exports = names("libobs/cmake/rust-exports.d");
    let hidden = names("libobs/cmake/rust-hidden.d");
    assert!(hidden.iter().any(|name| name == SHIM));
    for name in std::iter::once(SHIM).chain(ADAPTERS) {
        assert!(
            !exports.iter().any(|export| export == name),
            "{name} exported"
        );
    }
    let internal = fs::read_to_string(root.join("libobs/obs-internal.h")).unwrap();
    assert!(internal.contains(&format!(
        "extern uint32_t {SHIM}(struct obs_encoder_group *group);"
    )));
    let fields = fs::read_to_string(root.join(FIELDS)).unwrap();
    assert!(!fields.contains("EXPORT"));
    for name in ADAPTERS {
        assert!(fields.contains(name), "{name} missing from {FIELDS}");
        assert!(
            !internal.contains(name),
            "{name} declared in obs-internal.h"
        );
    }
}

/// The record support, the ported group, the hybrid group and the hidden list.
fn files_are_wired(root: &Path, record: &str) {
    let fields = format!("#include \"{FIELDS}\"");
    let record_text = fs::read_to_string(root.join(record)).unwrap();
    assert!(record_text.contains(&fields));
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/encoder-group-num-started.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-encoder-group-num-started+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-encoder-group-num-started+.h.c"))
            .unwrap();
    let original = format!("#include \"{ORIGINAL}\"");
    assert!(group.contains(&original));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-output+.h.c")).unwrap();
    assert!(!hybrid.contains(&original));
    // The C-owned mutex and count stay in the core group.
    assert!(hybrid.contains(&fields));
    let hidden = fs::read_to_string(
        root.join("libobs/cmake/rust-hidden.d/obs-output-encoder_group_num_started.txt"),
    )
    .unwrap();
    let names: Vec<_> = hidden
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .collect();
    assert_eq!(names, [SHIM]);
}

use super::*;

const ORIGINAL: &str = "libobs/obs-service-info-accessors.c.h";
const FIELDS: &str = "libobs/obs-service-info-accessors-fields.c.h";
const SUPPORT: &str = "test/cmocka/service-info-accessors-link-support.c";
const SHIMS: [&str; 4] = [
    "obs_service_set_info",
    "obs_service_set_plugin_data",
    "obs_service_get_info_table",
    "obs_service_get_plugin_data",
];
const ADAPTER: &str = "obs_service_info_accessors_fields";

/// The unchanged characterization keeps testing the original C source; its
/// twin compiles the same fixture with OBS_SERVICE_INFO_ACCESSORS_RUST, so
/// the four accessors come from the Rust archive and reach the service
/// through the real field adapter.
#[test]
fn canonical_service_info_accessors_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let c_test = "test_service_info_accessors";
    let rust_test = "test_service_info_accessors_rust";
    graph
        .targets
        .retain(|key, _| key == c_test || key == rust_test);
    graph
        .tests
        .retain(|key, _| key == c_test || key == rust_test);
    let test_source = "test/cmocka/test_service_info_accessors.c";
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
                [Source::Path(a), Source::Path(b)] if a == test_source && b == SUPPORT));
            assert_eq!(target.definitions, ["OBS_SERVICE_INFO_ACCESSORS_RUST"]);
            assert_eq!(selected.tests[rust_test].target, rust_test);
        }
    }
    // The fixture includes the original only without the Rust definition.
    let fixture = fs::read_to_string(root.join(test_source)).unwrap();
    assert!(fixture.contains("#ifndef OBS_SERVICE_INFO_ACCESSORS_RUST"));
    files_are_wired(&root);
}

/// The shims are hidden in both libraries, as the C was, and the adapter is
/// never exported, so a Darwin twin cannot bind either from libobs.dylib
/// (#5226).
#[test]
fn canonical_service_info_accessors_names_stay_private() {
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
    for name in SHIMS.into_iter().chain([ADAPTER]) {
        assert!(
            !exports.iter().any(|export| export == name),
            "{name} exported"
        );
    }
    let hidden = fs::read_to_string(
        root.join("libobs/cmake/rust-hidden.d/obs-output-service_info_accessors.txt"),
    )
    .unwrap();
    let listed: Vec<_> = hidden
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .collect();
    assert_eq!(listed, SHIMS);
    let fields = fs::read_to_string(root.join(FIELDS)).unwrap();
    assert!(!fields.contains("EXPORT"));
    assert!(fields.contains(ADAPTER));
}

/// The support member, the ported group, the hybrid group.
fn files_are_wired(root: &Path) {
    let include = |path: &str| format!("#include \"{path}\"");
    let support = fs::read_to_string(root.join(SUPPORT)).unwrap();
    assert!(support.contains(&include(FIELDS)));
    assert!(!support.contains(&include(ORIGINAL)));
    let manifest =
        fs::read_to_string(root.join("test/cmocka/build/rust-static-support.json")).unwrap();
    assert!(manifest.contains(&format!("\"{SUPPORT}\"")));
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/service-info-accessors.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-service-info-accessors+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-service-info-accessors+.h.c"))
            .unwrap();
    assert!(group.contains(&include(ORIGINAL)));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-output+.h.c")).unwrap();
    assert!(!hybrid.contains(&include(ORIGINAL)));
    // The C-owned fields stay in the core group.
    assert!(hybrid.contains(&include(FIELDS)));
}

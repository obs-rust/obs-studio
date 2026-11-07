use super::*;

/// #1390: the conversion table must exercise the production Rust archive.
#[test]
fn canonical_rust_static_conversion_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_async_conversion_plan_rust";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits Rust-static conversion");
    assert_source_getter_link_support(target, "test_async_conversion_plan", false);
    assert_eq!(target.includes, ["libobs"]);
    assert_eq!(target.watch_inputs, ["libobs/obs-source-conversion-plan.h"]);
    assert_rust_static_conversion_linkage(target);
    assert!(target.archives.is_empty());
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_async_conversion_plan.cmake"))
            .unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in [
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ] {
        for available in [false, true] {
            let msvc = os.contains("msvc");
            let facts = [
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
                ("MSVC".into(), msvc),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(selected.targets.contains_key(name), available, "{os}");
            assert_eq!(selected.tests.contains_key(name), available, "{os}");
        }
    }
}

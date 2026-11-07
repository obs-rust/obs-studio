use super::*;

/// #1390: preserve the direct Rust-static characterization, without a C core.
#[test]
fn canonical_rust_static_pacing_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_async_frame_pacing_steps_rust";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits Rust-static pacing");
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_async_frame_pacing_steps.c"));
    assert_eq!(target.includes, ["libobs"]);
    assert_eq!(target.watch_inputs, ["libobs/obs-source-async-pacing.h"]);
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("Rust-static pacing linkage must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["libobs_rust-static", "OBS::native-test-cmocka"]);
    assert!(target.archives.is_empty());
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_async_frame_pacing_steps.cmake"))
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
            let facts = [
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
                ("OS_LINUX".into(), os.contains("linux")),
                ("OS_MACOS".into(), os.contains("apple")),
                ("OS_WINDOWS".into(), os.contains("windows")),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(selected.targets.contains_key(name), available, "{os}");
            assert_eq!(selected.tests.contains_key(name), available, "{os}");
        }
    }
}

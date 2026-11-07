use super::*;

/// #1390: the X11 reply-bit helper needs headers, not a display or XCB linkage.
#[test]
fn canonical_x11_keymap_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_x11_keymap";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits test_x11_keymap");
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_x11_keymap.c"));
    assert_eq!(target.includes, ["libobs"]);
    assert_eq!(target.watch_inputs, ["libobs/obs-nix-x11-keymap.h"]);
    assert_eq!(target.header_packages, ["xcb"]);
    assert!(target.packages.is_empty());
    assert!(target.header_libraries.is_empty());
    assert!(
        matches!(target.libraries.as_slice(), [ConditionalValue::Name(library)]
        if library == "OBS::native-test-cmocka")
    );
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.suite, "core");
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_x11_keymap.cmake")).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in [
        "OS_LINUX",
        "OS_FREEBSD",
        "OS_MACOS",
        "OS_WINDOWS",
        "OS_OPENBSD",
        "OTHER",
    ] {
        let selected = graph
            .select(
                &[
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_FREEBSD".into(), os == "OS_FREEBSD"),
                ]
                .into(),
            )
            .unwrap();
        let expected = matches!(os, "OS_LINUX" | "OS_FREEBSD");
        assert_eq!(selected.targets.contains_key(name), expected, "{os}");
        assert_eq!(selected.tests.contains_key(name), expected, "{os}");
    }
}

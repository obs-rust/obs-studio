use super::*;

/// #1390: preserve the unchanged, GPU-free VAAPI fallback policy and OS guard.
#[test]
fn canonical_vaapi_device_select_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    let name = "test_vaapi_device_select";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits test_vaapi_device_select");
    let sources: Vec<_> = target
        .sources
        .iter()
        .map(|source| match source {
            Source::Path(path) => path.as_str(),
            _ => panic!("VAAPI fallback sources must be unconditional"),
        })
        .collect();
    assert_eq!(
        sources,
        [
            "test/cmocka/test_vaapi_device_select.c",
            "plugins/obs-ffmpeg/vaapi-device-select.c.h"
        ]
    );
    assert_eq!(target.includes, ["plugins/obs-ffmpeg"]);
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("VAAPI fallback linkage must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::native-test-cmocka"]);
    assert!(target.header_libraries.is_empty());
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.suite, "core");
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert!(test.resource_locks.is_empty());
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_vaapi_device_select.cmake"))
            .unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in [
        "OS_LINUX",
        "OS_FREEBSD",
        "OS_OPENBSD",
        "OS_MACOS",
        "OS_WINDOWS",
        "OS_OTHER",
    ] {
        let facts = ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD"]
            .map(|fact| (fact.into(), fact == os))
            .into();
        let selected = graph.select(&facts).unwrap();
        let enabled = matches!(os, "OS_LINUX" | "OS_FREEBSD" | "OS_OPENBSD");
        assert_eq!(selected.targets.contains_key(name), enabled, "{os}");
        assert_eq!(selected.tests.contains_key(name), enabled, "{os}");
    }
}

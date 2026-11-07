use super::*;

/// #3677: both native consumers retain all four missing-files lifecycle cases.
#[test]
fn canonical_missing_files_list_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_missing_files_list";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits missing-files list compatibility coverage");
    assert_eq!(
        target
            .sources
            .iter()
            .map(|source| match source {
                Source::Path(path) => path.as_str(),
                _ => panic!("missing-files list sources must be unconditional"),
            })
            .collect::<Vec<_>>(),
        [
            "test/cmocka/test_missing_files_list.c",
            "frontend/utility/missing-files-list.c.h"
        ]
    );
    assert_eq!(target.includes, ["frontend/utility"]);
    assert_eq!(
        target
            .libraries
            .iter()
            .map(|library| match library {
                ConditionalValue::Name(name) => name.as_str(),
                _ => panic!("missing-files list libraries must be unconditional"),
            })
            .collect::<Vec<_>>(),
        ["OBS::libobs", "OBS::native-test-cmocka"]
    );
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert!(
        !root
            .join("test/cmocka/tests.d/test_missing_files_list.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}

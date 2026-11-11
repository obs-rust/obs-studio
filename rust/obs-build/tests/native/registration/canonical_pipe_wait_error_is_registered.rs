use super::*;

/// #1390: preserve real libobs linkage and the Linux waitpid interposition test.
#[test]
fn canonical_pipe_wait_error_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.retain_suite("core");
    let name = "test_pipe_wait_error";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits test_pipe_wait_error");
    let sources: Vec<_> = target
        .sources
        .iter()
        .map(|source| match source {
            Source::Path(path) => path.as_str(),
            _ => panic!("pipe wait sources must be unconditional"),
        })
        .collect();
    assert_eq!(
        sources,
        [
            "test/cmocka/test_pipe_wait_error.c",
            "libobs/util/pipe-posix.c.h"
        ]
    );
    assert!(
        matches!(target.libraries.as_slice(), [ConditionalValue::Name(name)] if name == "OBS::libobs")
    );
    assert!(target.header_libraries.is_empty());
    assert!(target.packages.is_empty());
    assert_eq!(
        target.watch_inputs,
        ["libobs/build/libobs-ported-posix.pch.h"]
    );
    assert_eq!(target.link_options, ["-Wl,--wrap=waitpid"]);
    assert!(target.compile_options.is_empty());
    assert_eq!(
        target.compile_options_if_supported,
        ["-fvisibility=default"]
    );
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, Some(30));
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/pipe_wait_error.cmake")).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in [
        "OS_LINUX",
        "OS_MACOS",
        "OS_WINDOWS",
        "OS_FREEBSD",
        "OS_OPENBSD",
    ] {
        let facts = [("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into();
        let selected = graph.select(&facts).unwrap();
        assert_eq!(
            selected.targets.contains_key(name),
            os == "OS_LINUX",
            "{os}"
        );
        assert_eq!(selected.tests.contains_key(name), os == "OS_LINUX", "{os}");
    }
}

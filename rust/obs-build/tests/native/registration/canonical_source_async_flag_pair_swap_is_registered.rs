use super::*;

#[test]
fn canonical_source_async_flag_pair_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_async_flag_pair";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        // The unchanged test compiles no copy of the setter or the getter: it
        // calls the exported symbols of the real libobs (Rust in hybrid, the
        // original C in the reference).
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_source_async_flag_pair.c")
        );
        assert!(selected.targets[name].definitions.is_empty());
        assert_eq!(selected.tests[name].target, name);
    }
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    for (set, group, file, fields, exports, names) in [
        (
            "source-async-flag-getters",
            "libobs.ported-source-async-flag-getters+.h.c",
            "obs-source-async-unbuffered-setter.c.h",
            "obs-source-async-flag-fields.c.h",
            "obs-source-source_async_flag_getters.txt",
            &[
                "obs_source_async_unbuffered",
                "obs_source_async_decoupled",
                "obs_source_set_async_unbuffered",
            ][..],
        ),
        (
            "source-async-compensation",
            "libobs.ported-source-async-compensation+.h.c",
            "obs-source-async-compensation-getter.c.h",
            "obs-source-async-compensation-fields.c.h",
            "obs-source-source_async_compensation.txt",
            &[
                "obs_source_set_async_compensation",
                "obs_source_async_compensation",
            ][..],
        ),
    ] {
        let ported =
            fs::read_to_string(root.join(format!("libobs/build/ported-source-sets/{set}.json")))
                .unwrap();
        assert!(ported.contains(&format!("build/{group}")));
        let include = format!("#include \"libobs/{file}\"");
        let text = fs::read_to_string(root.join(format!("libobs/build/{group}"))).unwrap();
        assert!(text.contains(&include));
        assert!(!hybrid.contains(&include));
        assert!(hybrid.contains(&format!("#include \"libobs/{fields}\"")));
        let listed =
            fs::read_to_string(root.join(format!("libobs/cmake/rust-exports.d/{exports}")))
                .unwrap();
        assert_eq!(listed.lines().collect::<Vec<_>>(), names);
    }
}

use super::*;

#[test]
fn canonical_source_muted_volume_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_muted_volume";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    // The test compiles no copy of the steps: the capture path and the
    // audio mix of the real libobs reach the hidden symbols (Rust in
    // hybrid, the original C in the reference). The mix needs the OpenGL
    // core, so only Linux selects it.
    let selected = graph
        .select(&[("OS_LINUX".into(), true), ("HAVE_OBS_OPENGL".into(), true)].into())
        .unwrap();
    assert!(
        matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_source_muted_volume.c")
    );
    assert!(selected.targets[name].definitions.is_empty());
    assert_eq!(selected.tests[name].target, name);
    for os in ["OS_MACOS", "OS_WINDOWS"] {
        let facts = [
            (os.into(), true),
            ("OS_LINUX".into(), false),
            ("HAVE_OBS_OPENGL".into(), true),
        ];
        let selected = graph.select(&facts.into()).unwrap();
        assert!(selected.targets.is_empty(), "{os}");
    }
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/source-muted-volume.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-source-muted-volume+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-muted-volume+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-muted-volume.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-muted-volume.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-muted-volume-fields.c.h\""));
    // obs-source.c.h keeps the callers and no copy of the steps.
    let source = fs::read_to_string(root.join("libobs/obs-source.c.h")).unwrap();
    assert!(source.contains("get_source_volume(source, source->audio_ts)"));
    let hidden = fs::read_to_string(
        root.join("libobs/cmake/rust-hidden.d/obs-source-source_muted_volume.txt"),
    )
    .unwrap();
    for step in ["source_muted", "get_source_volume"] {
        assert!(
            !source.contains(&format!(
                " {step}(obs_source_t *source, uint64_t os_time)\n{{"
            )),
            "{step}"
        );
        assert!(hidden.lines().any(|line| line == step), "{step}");
        // Hidden, not exported: obs-internal.h declares them without EXPORT.
        for entry in fs::read_dir(root.join("libobs/cmake/rust-exports.d")).unwrap() {
            let text = fs::read_to_string(entry.unwrap().path()).unwrap();
            assert!(!text.lines().any(|line| line == step), "{step}");
        }
    }
}

use super::*;

#[test]
fn canonical_canvas_valid_video_info_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let c_test = "test_canvas_valid_video_info";
    let rust_test = "test_canvas_valid_video_info_rust";
    graph
        .targets
        .retain(|key, _| key == c_test || key == rust_test);
    graph
        .tests
        .retain(|key, _| key == c_test || key == rust_test);
    let test_source = "test/cmocka/test_canvas_valid_video_info.c";
    let rules = "libobs/obs-canvas-rules.c.h";
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for available in [false, true] {
            let facts = [
                (os.into(), true),
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            // The unchanged characterization keeps testing the original C
            // source over the original C rules.
            assert!(matches!(selected.targets[c_test].sources.as_slice(),
                [Source::Path(a), Source::Path(b)] if a == test_source && b == rules));
            assert!(selected.targets[c_test].definitions.is_empty());
            assert_eq!(selected.tests[c_test].target, c_test);
            assert_eq!(selected.targets.contains_key(rust_test), available);
            if !available {
                continue;
            }
            // Its external seam reaches the Rust shim, which reads the real
            // canvas through the shared ovi adapter; no C rules are compiled.
            let target = &selected.targets[rust_test];
            assert!(
                matches!(target.sources.as_slice(), [Source::Path(path)] if path == test_source)
            );
            assert_eq!(target.definitions, ["CANVAS_VALID_INFO_EXTERNAL"]);
            assert_eq!(selected.tests[rust_test].target, rust_test);
        }
    }
    let test_text = fs::read_to_string(root.join(test_source)).unwrap();
    assert!(test_text.contains("#ifndef CANVAS_VALID_INFO_EXTERNAL"));
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/canvas-valid-video-info.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-canvas-valid-video-info+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-canvas-valid-video-info+.h.c"))
            .unwrap();
    let original = "#include \"libobs/obs-canvas-has-valid-video-info.c.h\"";
    assert!(group.contains(original));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains(original));
    // The C-owned canvas layout stays behind the production adapter.
    let fields = "#include \"libobs/obs-canvas-has-valid-video-info-fields.c.h\"";
    assert!(hybrid.contains(fields));
    // Standalone Rust-static consumers get the same real adapter.
    let support =
        fs::read_to_string(root.join("test/cmocka/build/rust-static-support.json")).unwrap();
    assert!(support.contains("\"test/cmocka/canvas-valid-video-info-link-support.c\""));
    let link = fs::read_to_string(root.join("test/cmocka/canvas-valid-video-info-link-support.c"))
        .unwrap();
    assert!(link.contains(fields));
    let hidden = fs::read_to_string(
        root.join("libobs/cmake/rust-hidden.d/obs-core-canvas_valid_video_info.txt"),
    )
    .unwrap();
    let names: Vec<_> = hidden
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .collect();
    assert_eq!(names, ["obs_canvas_has_valid_video_info"]);
}

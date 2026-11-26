use super::*;

#[test]
fn canonical_source_get_color_space_swap_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/source-get-color-space.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-source-get-color-space+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-get-color-space+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-get-color-space.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-get-color-space.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-get-color-space-fields.c.h\""));
    let source = fs::read_to_string(root.join("libobs/obs-source.c.h")).unwrap();
    assert!(!source.contains("enum gs_color_space obs_source_get_color_space("));
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-source-source_get_color_space.txt"),
    )
    .unwrap();
    assert_eq!(exports.trim(), "obs_source_get_color_space");
}

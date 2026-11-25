use super::*;

#[test]
fn canonical_source_get_size_swap_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/source-get-size.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-source-get-size+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-get-size+.h.c")).unwrap();
    assert!(group.contains("#include \"libobs/obs-source-get-size.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-get-size.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-get-size-fields.c.h\""));
    let exports =
        fs::read_to_string(root.join("libobs/cmake/rust-exports.d/obs-source-source_get_size.txt"))
            .unwrap();
    assert_eq!(
        exports.lines().collect::<Vec<_>>(),
        ["obs_source_get_width", "obs_source_get_height"]
    );
}

use obs_scene::item_blending_method::set_method;

#[test]
fn test_scene_item_blending_method_null_item() {
    let mut diagnostics = 0;
    set_method(None, 0, || diagnostics += 1);
    assert_eq!(diagnostics, 1);
}

#[test]
fn test_scene_item_blending_method_stored_values() {
    for dirty in [false, true] {
        let mut state = (-17i64, true, true, 3u32, 2u32, dirty, 0u32);
        for method in [0, 1, 17, 0x7fff_ffff] {
            for _ in 0..2 {
                let mut expected = state;
                expected.6 = method;
                set_method(Some(&mut state.6), method, || panic!("live item"));
                assert_eq!(state, expected);
            }
        }
    }
}

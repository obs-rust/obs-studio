//! Real-header prefix layout and exact Tier 3 parity. No intended differences.
use core::mem::{align_of, offset_of, size_of};
use obs_c_oracle as _;
use obs_scene::ffi::item_render_getters::*;
use proptest::prelude::*;
type Getter = unsafe extern "C" fn(*const SceneItemRenderPrefix) -> u32;
unsafe extern "C" {
    fn oracle_scene_render_layout(field: u32) -> usize;
    fn oracle_scene_render_probe(
        scale: u32,
        method: u32,
        mode: u32,
        flags: u32,
        a: Getter,
        b: Getter,
        c: Getter,
    ) -> u32;
    fn oracle_scene_render_null_probe(a: Getter, b: Getter, c: Getter) -> u32;
}
#[test]
fn real_header_prefix_layout() {
    let rust = [
        size_of::<SceneItemRenderPrefix>(),
        align_of::<SceneItemRenderPrefix>(),
        offset_of!(SceneItemRenderPrefix, refs),
        offset_of!(SceneItemRenderPrefix, removed),
        offset_of!(SceneItemRenderPrefix, is_group),
        offset_of!(SceneItemRenderPrefix, is_scene),
        offset_of!(SceneItemRenderPrefix, update_transform),
        offset_of!(SceneItemRenderPrefix, update_group_resize),
        offset_of!(SceneItemRenderPrefix, id),
        offset_of!(SceneItemRenderPrefix, parent),
        offset_of!(SceneItemRenderPrefix, source),
        offset_of!(SceneItemRenderPrefix, active_refs),
        offset_of!(SceneItemRenderPrefix, defer_update),
        offset_of!(SceneItemRenderPrefix, defer_group_resize),
        offset_of!(SceneItemRenderPrefix, user_visible),
        offset_of!(SceneItemRenderPrefix, visible),
        offset_of!(SceneItemRenderPrefix, selected),
        offset_of!(SceneItemRenderPrefix, locked),
        offset_of!(SceneItemRenderPrefix, item_render),
        offset_of!(SceneItemRenderPrefix, crop),
        offset_of!(SceneItemRenderPrefix, absolute_coordinates),
        offset_of!(SceneItemRenderPrefix, pos),
        offset_of!(SceneItemRenderPrefix, scale),
        offset_of!(SceneItemRenderPrefix, scale_ref),
        offset_of!(SceneItemRenderPrefix, rot),
        offset_of!(SceneItemRenderPrefix, align),
        offset_of!(SceneItemRenderPrefix, last_width),
        offset_of!(SceneItemRenderPrefix, last_height),
        offset_of!(SceneItemRenderPrefix, output_scale),
        offset_of!(SceneItemRenderPrefix, scale_filter),
        offset_of!(SceneItemRenderPrefix, blend_method),
        offset_of!(SceneItemRenderPrefix, blend_type),
        size_of::<u32>(),
        align_of::<u32>(),
        size_of::<u32>(),
        align_of::<u32>(),
        size_of::<u32>(),
        align_of::<u32>(),
        size_of::<[i32; 4]>(),
        align_of::<[i32; 4]>(),
        size_of::<[f32; 2]>(),
        align_of::<[f32; 2]>(),
    ];
    for (i, value) in rust.into_iter().enumerate() {
        // SAFETY: oracle accepts every integer and returns only layout constants.
        assert_eq!(
            value,
            unsafe { oracle_scene_render_layout(i as u32) },
            "layout index {i}"
        );
    }
}
fn check(scale: u32, method: u32, mode: u32, flags: u32) {
    // SAFETY: C supplies complete initialized items for synchronous field readers.
    let mismatch = unsafe {
        oracle_scene_render_probe(
            scale,
            method,
            mode,
            flags,
            obs_sceneitem_get_scale_filter,
            obs_sceneitem_get_blending_method,
            obs_sceneitem_get_blending_mode,
        )
    };
    assert_eq!(
        mismatch, 0,
        "scale={scale} method={method} mode={mode} flags={flags} bits={mismatch}"
    );
}
#[test]
fn characterized_edges() {
    let values = [
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        17,
        0x7fff_ffff,
        0x8000_0000,
        u32::MAX,
    ];
    for scale in values {
        for method in values {
            for mode in values {
                for flags in [0, 1, 2, 4, 8, 15] {
                    check(scale, method, mode, flags);
                }
            }
        }
    }
}
proptest! {
 #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(1024), failure_persistence: None, ..ProptestConfig::default() })]
 #[test]
 fn arbitrary_values(scale in any::<u32>(), method in any::<u32>(), mode in any::<u32>(), flags in any::<u32>()) { check(scale,method,mode,flags); }
}
#[test]
fn null_diagnostics() {
    const CHILD: &str = "OBS_SCENE_RENDER_LOG_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // SAFETY: isolated one-test child; probe saves and restores global logging.
        assert_eq!(
            unsafe {
                oracle_scene_render_null_probe(
                    obs_sceneitem_get_scale_filter,
                    obs_sceneitem_get_blending_method,
                    obs_sceneitem_get_blending_mode,
                )
            },
            0
        );
        return;
    }
    let module = module_path!().split_once("::").unwrap().1;
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .env(CHILD, "1")
        .args([
            "--exact",
            &format!("{module}::null_diagnostics"),
            "--nocapture",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

//! Complete output/state byte parity against verbatim C, with no exclusions.
use core::mem::{align_of, offset_of, size_of};
use obs_c_oracle as _;
use obs_scene::ffi::item_crop_getter::{SceneItemPrefix, obs_sceneitem_get_crop};
use obs_scene::ffi::scene_bounds_place::SceneItemCrop;
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const SceneItemPrefix, *mut SceneItemCrop);
unsafe extern "C" {
    fn oracle_scene_crop_getter_layout(field: u32) -> usize;
    fn oracle_scene_crop_getter_probe(values: *const i32, flags: u32, getter: Getter) -> u32;
    fn oracle_scene_crop_getter_null(getter: Getter) -> u32;
    fn oracle_scene_crop_getter_concurrent(getter: Getter) -> u32;
}

#[test]
fn real_header_layout() {
    let layout = [
        size_of::<SceneItemPrefix>(),
        align_of::<SceneItemPrefix>(),
        offset_of!(SceneItemPrefix, state),
        offset_of!(SceneItemPrefix, item_render),
        offset_of!(SceneItemPrefix, crop),
        size_of::<SceneItemCrop>(),
        align_of::<SceneItemCrop>(),
        offset_of!(SceneItemCrop, left),
        offset_of!(SceneItemCrop, top),
        offset_of!(SceneItemCrop, right),
        offset_of!(SceneItemCrop, bottom),
    ];
    for (i, value) in layout.into_iter().enumerate() {
        // SAFETY: accepts any integer and returns a real-header measurement.
        assert_eq!(
            value,
            unsafe { oracle_scene_crop_getter_layout(i as u32) },
            "layout {i}"
        );
    }
}

fn check(values: [i32; 4], flags: u32) {
    // SAFETY: C owns and initializes complete live objects, guards all output
    // bytes, and calls the shim under the original lifecycle/lock contract.
    assert_eq!(
        unsafe { oracle_scene_crop_getter_probe(values.as_ptr(), flags, obs_sceneitem_get_crop) },
        0,
        "values={values:?}, flags={flags}"
    );
}

#[test]
fn characterized_edges() {
    let edges = [0, -1, 1, i32::MIN, i32::MAX];
    for left in edges {
        for top in edges {
            for right in edges {
                for bottom in edges {
                    for flags in 0..2 {
                        check([left, top, right, bottom], flags);
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_values(values in any::<[i32; 4]>(), flags in any::<u32>()) { check(values, flags); }
}

#[test]
fn parent_video_mutex_blocks_and_publishes() {
    // SAFETY: C joins each worker before releasing its complete C-owned state.
    assert_eq!(
        unsafe { oracle_scene_crop_getter_concurrent(obs_sceneitem_get_crop) },
        0
    );
}

#[test]
fn null_diagnostics() {
    const CHILD: &str = "OBS_SCENE_CROP_LOG_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // SAFETY: a one-test child isolates the saved/restored global log handler.
        assert_eq!(
            unsafe { oracle_scene_crop_getter_null(obs_sceneitem_get_crop) },
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

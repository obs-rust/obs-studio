//! Tier 3: whole-item byte parity against the verbatim C setter, with the
//! original C clamp and the same item fields lock. No intentional
//! differences, so no input is excluded. The raw-input compare (a negative
//! input stores and marks every time) is characterized, not endorsed.
use core::mem::{offset_of, size_of};
use obs_c_oracle as _;
use obs_scene::ffi::item_crop_getter::SceneItemPrefix;
use obs_scene::ffi::item_crop_setter::obs_sceneitem_set_crop;
use obs_scene::ffi::item_state_getters::SceneItemPrefix as StatePrefix;
use obs_scene::ffi::scene_bounds_place::SceneItemCrop;
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut SceneItemPrefix, *const SceneItemCrop);
unsafe extern "C" {
    fn oracle_scene_crop_setter_layout(field: u32) -> usize;
    fn oracle_scene_crop_setter_probe(
        stored: *const i32,
        input: *const i32,
        flags: u32,
        setter: Setter,
    ) -> u32;
    fn oracle_scene_crop_setter_null(setter: Setter) -> u32;
    fn oracle_scene_crop_setter_concurrent(setter: Setter) -> u32;
}

#[test]
fn real_header_layout() {
    let layout = [
        offset_of!(SceneItemPrefix, state) + offset_of!(StatePrefix, update_transform),
        size_of::<bool>(),
    ];
    for (i, value) in layout.into_iter().enumerate() {
        // SAFETY: accepts any integer and returns a real-header measurement.
        assert_eq!(
            value,
            unsafe { oracle_scene_crop_setter_layout(i as u32) },
            "layout {i}"
        );
    }
}

fn check(stored: [i32; 4], input: [i32; 4], flags: u32) {
    // SAFETY: C owns and initializes complete live items and the parent, and
    // calls the shim under the original lifecycle and lock contract.
    assert_eq!(
        unsafe {
            oracle_scene_crop_setter_probe(
                stored.as_ptr(),
                input.as_ptr(),
                flags,
                obs_sceneitem_set_crop,
            )
        },
        0,
        "stored={stored:?}, input={input:?}, flags={flags}"
    );
}

#[test]
fn characterized_edges() {
    let edges = [0, -1, 1, i32::MIN, i32::MAX];
    let crops: Vec<[i32; 4]> = (0..edges.len())
        .flat_map(|i| {
            (0..edges.len())
                .map(move |j| [edges[i], edges[j], edges[(i + j) % 5], edges[(i + 2) % 5]])
        })
        .collect();
    for stored in &crops {
        for input in &crops {
            for flags in 0..8 {
                check(*stored, *input, flags);
            }
        }
        // The raw input equal to the stored crop, and its clamp.
        for flags in 0..8 {
            check(*stored, *stored, flags);
            check(stored.map(|v| v.max(0)), *stored, flags);
        }
    }
}

fn crop() -> impl Strategy<Value = [i32; 4]> {
    prop_oneof![
        any::<[i32; 4]>(),
        proptest::array::uniform4(prop_oneof![Just(0), Just(-1), Just(i32::MIN), -3..4i32]),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_crops(stored in crop(), input in crop(), same in any::<bool>(), flags in 0..8u32) {
        check(stored, if same { stored } else { input }, flags);
    }
}

#[test]
fn parent_video_mutex_blocks_then_marks_after_store() {
    // SAFETY: C joins each worker before releasing its complete C-owned state.
    assert_eq!(
        unsafe { oracle_scene_crop_setter_concurrent(obs_sceneitem_set_crop) },
        0
    );
}

#[test]
fn null_diagnostics() {
    const CHILD: &str = "OBS_SCENE_CROP_SETTER_LOG_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // SAFETY: a one-test child isolates the saved/restored global log handler.
        assert_eq!(
            unsafe { oracle_scene_crop_setter_null(obs_sceneitem_set_crop) },
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

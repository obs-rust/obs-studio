//! Tier 3: the unchanged extracted C getter over real-header items.
//! No intentional behavior differences or excluded inputs. Settings handles are
//! compared by identity and reference count, never by address arithmetic; the
//! null log probe runs in an isolated child.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_c_oracle as _;
use obs_data::ffi::obs_data::{
    obs_data_create, obs_data_get_int, obs_data_release, obs_data_set_int, ref_count_for_tests,
};
use obs_scene::ffi::item_private_settings::{SceneItemPrefix, obs_sceneitem_get_private_settings};
use obs_scene::ffi::scene_bounds_place::SceneItemCrop;
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const SceneItemPrefix) -> *mut c_void;
unsafe extern "C" {
    fn oracle_scene_item_private_settings_layout(field: u32) -> usize;
    fn oracle_scene_item_private_settings_new(settings: *mut c_void, fill: u8) -> *mut c_void;
    fn oracle_scene_item_private_settings_unchanged(item: *const c_void) -> bool;
    fn oracle_scene_item_private_settings_free(item: *mut c_void);
    fn oracle_obs_sceneitem_get_private_settings(item: *const SceneItemPrefix) -> *mut c_void;
    fn oracle_scene_item_private_settings_null_probe(getter: Getter) -> u32;
}

#[test]
fn real_header_prefix_layout() {
    let rust = [
        size_of::<SceneItemPrefix>(),
        align_of::<SceneItemPrefix>(),
        offset_of!(SceneItemPrefix, state),
        offset_of!(SceneItemPrefix, item_render),
        offset_of!(SceneItemPrefix, crop),
        offset_of!(SceneItemPrefix, absolute_coordinates),
        offset_of!(SceneItemPrefix, pos),
        offset_of!(SceneItemPrefix, scale),
        offset_of!(SceneItemPrefix, scale_ref),
        offset_of!(SceneItemPrefix, rot),
        offset_of!(SceneItemPrefix, align),
        offset_of!(SceneItemPrefix, last_width),
        offset_of!(SceneItemPrefix, last_height),
        offset_of!(SceneItemPrefix, output_scale),
        offset_of!(SceneItemPrefix, scale_filter),
        offset_of!(SceneItemPrefix, blend_method),
        offset_of!(SceneItemPrefix, blend_type),
        offset_of!(SceneItemPrefix, box_transform),
        offset_of!(SceneItemPrefix, box_scale),
        offset_of!(SceneItemPrefix, draw_transform),
        offset_of!(SceneItemPrefix, bounds_type),
        offset_of!(SceneItemPrefix, bounds_align),
        offset_of!(SceneItemPrefix, bounds),
        offset_of!(SceneItemPrefix, crop_to_bounds),
        offset_of!(SceneItemPrefix, bounds_crop),
        offset_of!(SceneItemPrefix, toggle_visibility),
        offset_of!(SceneItemPrefix, private_settings),
        size_of::<usize>(),
        size_of::<SceneItemCrop>(),
    ];
    for (index, value) in rust.into_iter().enumerate() {
        // SAFETY: the oracle accepts every integer and returns a layout value.
        assert_eq!(
            value,
            unsafe { oracle_scene_item_private_settings_layout(index as u32) },
            "layout index {index}"
        );
    }
}

struct Fixture {
    item: *mut c_void,
    settings: *mut c_void,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: the fixture and its initial settings reference are owned here.
        unsafe {
            oracle_scene_item_private_settings_free(self.item);
            obs_data_release(self.settings);
        }
    }
}

fn check(present: bool, fill: u8, value: i64, extra_refs: u8) {
    // SAFETY: each fixture is a complete real-layout item holding a live
    // settings object or NULL. Each call adds one reference, released again.
    unsafe {
        let settings = if present {
            obs_data_create()
        } else {
            core::ptr::null_mut()
        };
        if present {
            assert!(!settings.is_null());
        }
        obs_data_set_int(settings, c"value".as_ptr(), value);
        let fixture = Fixture {
            item: oracle_scene_item_private_settings_new(settings, fill),
            settings,
        };
        assert!(!fixture.item.is_null());
        let item = fixture.item.cast::<SceneItemPrefix>();
        let mut held = Vec::new();
        for _ in 0..extra_refs {
            held.push(obs_sceneitem_get_private_settings(item));
        }
        assert_eq!(
            ref_count_for_tests(settings),
            usize::from(present) * (1 + usize::from(extra_refs))
        );
        for getter in [
            obs_sceneitem_get_private_settings as Getter,
            oracle_obs_sceneitem_get_private_settings as Getter,
        ] {
            let before = ref_count_for_tests(settings);
            let result = getter(item);
            assert_eq!(result, settings);
            assert_eq!(ref_count_for_tests(settings), before + usize::from(present));
            assert_eq!(
                obs_data_get_int(result, c"value".as_ptr()),
                if present { value } else { 0 }
            );
            assert!(oracle_scene_item_private_settings_unchanged(fixture.item));
            obs_data_release(result);
            assert_eq!(ref_count_for_tests(settings), before);
        }
        for settings in held {
            obs_data_release(settings);
        }
        assert_eq!(ref_count_for_tests(settings), usize::from(present));
    }
}

#[test]
fn characterized_edges() {
    for present in [false, true] {
        for fill in [0x00, 0xa5, 0xff] {
            for value in [i64::MIN, 0, 17, i64::MAX] {
                for extra_refs in [0, 1, 4] {
                    check(present, fill, value, extra_refs);
                }
            }
        }
    }
}

#[test]
fn null_diagnostic() {
    const CHILD: &str = "OBS_SCENE_ITEM_PRIVATE_SETTINGS_LOG_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // SAFETY: this child has no concurrent tests; the C probe saves and
        // restores the global log handler around synchronous getter calls.
        assert_eq!(
            unsafe {
                oracle_scene_item_private_settings_null_probe(obs_sceneitem_get_private_settings)
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
            &format!("{module}::null_diagnostic"),
            "--nocapture",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_settings_match_c(present in any::<bool>(), fill in any::<u8>(), value in any::<i64>(), extra_refs in 0u8..8) {
        check(present, fill, value, extra_refs);
    }
}

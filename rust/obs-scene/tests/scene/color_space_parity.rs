//! Tier 3: the unchanged C callback and real C adapter, with one query per call.
//! No intentional differences; preferred spaces are ignored, characterized, not endorsed.
#![allow(unsafe_code)]
use core::ffi::{c_int, c_void};
use obs_scene::{color_space::*, ffi::color_space::scene_video_get_color_space};
use proptest::prelude::*;

type Callback = extern "C" fn(*mut c_void, usize, *const c_int) -> c_int;
unsafe extern "C" {
    fn oracle_scene_color_space_probe(
        present: bool,
        video: c_int,
        count: usize,
        preferences: *const c_int,
        callback: Callback,
    ) -> u32;
    fn oracle_scene_color_space_constants(values: *mut c_int);
}

fn check(present: bool, video: i32, preferences: &[i32]) {
    let before = preferences.to_vec();
    let pointer = if preferences.is_empty() {
        core::ptr::null()
    } else {
        preferences.as_ptr()
    };
    // SAFETY: preferences is live throughout both synchronous callbacks; the C
    // fixture is thread-local and the production adapter writes only its local int.
    let result = unsafe {
        oracle_scene_color_space_probe(
            present,
            video,
            preferences.len(),
            pointer,
            scene_video_get_color_space,
        )
    };
    assert_eq!(
        result, 0,
        "present={present} video={video} prefs={preferences:?}"
    );
    assert_eq!(preferences, before);
}

#[test]
fn constants_and_enum_representation_match_real_headers() {
    let mut values = [0; 9];
    // SAFETY: C writes exactly nine int values into this array.
    unsafe { oracle_scene_color_space_constants(values.as_mut_ptr()) };
    assert_eq!(
        values,
        [
            GS_CS_SRGB,
            GS_CS_709_EXTENDED,
            VIDEO_CS_SRGB,
            VIDEO_CS_2100_PQ,
            VIDEO_CS_2100_HLG,
            size_of::<i32>() as i32,
            size_of::<i32>() as i32,
            align_of::<i32>() as i32,
            align_of::<i32>() as i32
        ]
    );
}

#[test]
fn test_scene_color_space_preferences_and_failure() {
    let lists: &[&[i32]] = &[
        &[],
        &[GS_CS_SRGB],
        &[GS_CS_709_EXTENDED],
        &[1, 3],
        &[GS_CS_SRGB, GS_CS_709_EXTENDED],
        &[GS_CS_709_EXTENDED, GS_CS_SRGB],
    ];
    for present in [false, true] {
        for video in [
            i32::MIN,
            -1,
            0,
            1,
            2,
            VIDEO_CS_SRGB,
            VIDEO_CS_2100_PQ,
            VIDEO_CS_2100_HLG,
            6,
            i32::MAX,
        ] {
            for preferences in lists {
                check(present, video, preferences);
            }
        }
    }
    // Both callbacks ignore count and the pointer, even an unusable list.
    // SAFETY: neither callback dereferences preferences, irrespective of count.
    assert_eq!(
        unsafe {
            oracle_scene_color_space_probe(
                true,
                VIDEO_CS_2100_PQ,
                usize::MAX,
                core::ptr::null(),
                scene_video_get_color_space,
            )
        },
        0
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn arbitrary_video_and_preferences(
        present in any::<bool>(), video in any::<i32>(),
        preferences in proptest::collection::vec(any::<i32>(), 0..17),
    ) {
        check(present, video, &preferences);
    }
}

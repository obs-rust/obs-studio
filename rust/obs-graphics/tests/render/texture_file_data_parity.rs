//! Tier 3: unchanged C orchestration and real C adapter with recording FFmpeg
//! dependencies. No intentional differences. The C recorder is thread-local.
use core::ffi::c_char;
use obs_graphics::ffi::texture_file_data::gs_create_texture_file_data;
use obs_graphics::texture_render::ColorFormat;
use proptest::prelude::*;

type Loader = unsafe extern "C" fn(*const c_char, *mut ColorFormat, *mut u32, *mut u32) -> *mut u8;
unsafe extern "C" {
    fn oracle_texture_file_data_run(loader: Option<Loader>, config: *const i32, out: *mut u32);
}
fn run(loader: Option<Loader>, config: [i32; 5]) -> [u32; 24] {
    let mut out = [0; 24];
    // SAFETY: recorder borrows these bounded arrays synchronously, and loader
    // has the unchanged public C signature and writable outputs.
    unsafe { oracle_texture_file_data_run(loader, config.as_ptr(), out.as_mut_ptr()) };
    assert_eq!(out[5], 0, "unbalanced pixel/session allocation");
    assert_eq!(
        out[6], 0,
        "callback identity, ordering, output or pixels differ"
    );
    out
}
#[test]
fn characterization_matches_original_c() {
    for _ in 0..8 {
        for (init, decode) in [(0, 0), (1, 0), (1, 1)] {
            let config = [init, decode, 2, 1, 3];
            let original = run(None, config);
            assert_eq!(run(Some(gs_create_texture_file_data), config), original);
            assert_eq!(original[3], (init != 0 && decode != 0) as u32);
            assert_eq!(original[4], 1, "returned allocation identity");
            let events: &[u32] = if init == 0 {
                &[1]
            } else if decode == 0 {
                &[1, 2, 4]
            } else {
                &[1, 2, 3, 4]
            };
            assert_eq!(&original[8..8 + original[7] as usize], events);
        }
    }
}
#[test]
fn dimensions_and_formats_match_original_c() {
    for size in [i32::MIN, -1, 0, 1, i32::MAX] {
        for format in [0, 3, 5, i32::MAX] {
            let config = [1, 1, size, size.wrapping_add(1), format];
            assert_eq!(
                run(Some(gs_create_texture_file_data), config),
                run(None, config)
            );
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn arbitrary_sessions_match_original_c(init in any::<bool>(), decode in any::<bool>(), width in any::<i32>(), height in any::<i32>(), format in 0i32..=i32::MAX) {
        let config = [i32::from(init), i32::from(decode), width, height, format];
        prop_assert_eq!(run(Some(gs_create_texture_file_data), config), run(None, config));
    }
}

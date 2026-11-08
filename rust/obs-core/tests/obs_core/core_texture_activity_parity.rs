//! Exact parity for every nullable owner and both independent stored flags.
use obs_core::ffi::core_texture_activity::{Fields, obs_nv12_tex_active, obs_p010_tex_active};
use proptest::prelude::*;
use std::mem::{align_of, offset_of, size_of};
use std::sync::Mutex;
static LOCK: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn oracle_core_texture_activity_set(depth: u32, nv12: bool, p010: bool);
    fn oracle_core_texture_activity_unchanged() -> bool;
    fn oracle_core_texture_activity_layout(layout: *mut usize);
    fn oracle_obs_nv12_tex_active() -> bool;
    fn oracle_obs_p010_tex_active() -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: the C oracle writes exactly six size_t values.
    unsafe { oracle_core_texture_activity_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, nv12),
            offset_of!(Fields, p010),
            size_of::<bool>(),
            align_of::<bool>()
        ]
    );
}

fn compare(depth: u32, nv12: bool, p010: bool) {
    // SAFETY: LOCK isolates the real-header fixture, which remains live for
    // both implementations. Neither passes null to any field accessor.
    unsafe {
        oracle_core_texture_activity_set(depth, nv12, p010);
        for _ in 0..2 {
            assert_eq!(obs_nv12_tex_active(), oracle_obs_nv12_tex_active());
            assert_eq!(obs_p010_tex_active(), oracle_obs_p010_tex_active());
            assert_eq!(obs_nv12_tex_active(), depth == 3 && nv12);
            assert_eq!(obs_p010_tex_active(), depth == 3 && p010);
        }
        assert!(oracle_core_texture_activity_unchanged());
    }
}

#[test]
fn all_owner_and_flag_combinations() {
    let _guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for depth in 0..=3 {
        for nv12 in [false, true] {
            for p010 in [false, true] {
                compare(depth, nv12, p010);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn matches_original_c(depth in 0_u32..=3, nv12 in any::<bool>(), p010 in any::<bool>()) {
        let _guard = LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        compare(depth, nv12, p010);
        compare(0, false, false);
    }
}

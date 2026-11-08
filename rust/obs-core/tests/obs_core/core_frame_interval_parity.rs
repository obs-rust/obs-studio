//! Exact scalar parity. The original C requires an initialized core; every
//! fixture is live and non-null. The Rust shim also rejects null before access.
use obs_core::ffi::core_frame_interval::obs_get_frame_interval_ns;
use proptest::prelude::*;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn oracle_core_frame_interval_set(value: u64);
    fn oracle_core_frame_interval_unchanged() -> bool;
    fn oracle_core_frame_interval_layout(layout: *mut usize);
    fn oracle_obs_get_frame_interval_ns() -> u64;
}

#[test]
fn real_header_scalar_layout() {
    let mut layout = [0; 2];
    // SAFETY: the oracle writes exactly two size_t entries into the buffer.
    unsafe { oracle_core_frame_interval_layout(layout.as_mut_ptr()) };
    assert_eq!(layout, [size_of::<u64>(), align_of::<u64>()]);
}

fn compare(value: u64) {
    // SAFETY: LOCK isolates the fixture. Both implementations read the same
    // initialized real-header core; no accessor receives a null pointer.
    unsafe {
        oracle_core_frame_interval_set(value);
        assert_eq!(oracle_obs_get_frame_interval_ns(), value);
        assert_eq!(obs_get_frame_interval_ns(), value);
        assert_eq!(
            obs_get_frame_interval_ns(),
            oracle_obs_get_frame_interval_ns()
        );
        assert!(oracle_core_frame_interval_unchanged());
    }
}

#[test]
fn characterized_and_boundary_values() {
    let _guard = LOCK.lock().unwrap();
    for value in [0, 0, 0, 0, 1, 16_666_666, 33_366_666, u64::MAX, 0] {
        compare(value);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn interval_matches_original_c(value in any::<u64>()) {
        let _guard = LOCK.lock().unwrap();
        compare(value);
        compare(0);
    }
}

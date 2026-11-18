//! Tier 3: original C getter/weak upgrade against Rust with real C sources.
//! NULL source, dead controls and overflowing counts are C UB and excluded.
//! No defined-input differences. Existing canvas-ref parity owns control layout.
use core::ffi::{c_long, c_void};
use obs_source::ffi::source_canvas_getter::obs_source_get_canvas;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_canvas_check(
        getter: unsafe extern "C" fn(*const c_void) -> *mut c_void,
        refs: c_long,
        weak_refs: c_long,
        flags: u32,
        noise: u32,
    ) -> bool;
}

/// `c_long` is 32-bit on Windows and 64-bit on Linux/macOS; taking it directly
/// keeps the call free of a conversion that is useless on one of them.
const MAX_REFS: c_long = 100_000;

fn check(refs: c_long, weak_refs: c_long, flags: u32, noise: u32) {
    // SAFETY: oracle owns real-header source/control fixtures, keeps live strong
    // owners, balances every returned reference and checks all object bytes.
    assert!(
        unsafe { oracle_source_canvas_check(obs_source_get_canvas, refs, weak_refs, flags, noise) },
        "refs={refs}, weak_refs={weak_refs}, flags={flags}, noise={noise}"
    );
}

#[test]
fn owned_returns_moves_and_expiry_match_original_c() {
    for refs in [-1, 0, 1, 100_000] {
        for weak_refs in [0, 1, 100_000] {
            for flags in 0..4 {
                for noise in [0, 1, u32::MAX] {
                    check(refs, weak_refs, flags, noise);
                }
            }
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(-1..MAX_REFS, 0..MAX_REFS, 0u32..4, any::<u32>()),
            |(refs, weak_refs, flags, noise)| {
                check(refs, weak_refs, flags, noise);
                Ok(())
            },
        )
        .unwrap();
}

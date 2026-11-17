//! Tier 3: actual Rust exports vs original C, using the real private source layout.
use core::ffi::c_void;
use obs_source::ffi::source_save_load::{obs_source_load, obs_source_load2, obs_source_save};
use proptest::prelude::*;

type Entry = unsafe extern "C" fn(*mut c_void);
unsafe extern "C" {
    fn oracle_source_save_load_run(entry: Option<Entry>, op: u32, bits: u32, count: u32) -> u64;
}
fn compare(op: u32, bits: u32, count: u32) {
    let entry: Entry = [obs_source_save, obs_source_load, obs_source_load2][op as usize];
    // SAFETY: C owns the live real-layout fixture for the complete synchronous invocation.
    let (rust, original) = unsafe {
        (
            oracle_source_save_load_run(Some(entry), op, bits, count),
            oracle_source_save_load_run(None, op, bits, count),
        )
    };
    assert_eq!(rust, original, "op={op} bits={bits:#x} filters={count}");
}
#[test]
fn save_load_edges() {
    for bits in [
        0,
        u32::MAX,
        0xfffff,
        0xc01ff,
        0xc03ff,
        0x801ff,
        0x401ff,
        0x2c01ff,
        0x4c01ff,
        1,
    ] {
        for op in 0..3 {
            for count in 0..9 {
                compare(op, bits, count);
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn source_save_load_matches_c(op in 0u32..3, bits in any::<u32>(), count in 0u32..9) {
        compare(op, bits, count);
    }
}

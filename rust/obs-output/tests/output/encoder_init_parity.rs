//! Original extracted C against the real C bridge and Rust sequencer.
use obs_c_oracle as _;
use obs_output::ffi::encoder_init::{Fields, obs_encoder_init_steps};
use proptest::prelude::*;
use std::mem::{align_of, offset_of, size_of};

unsafe extern "C" {
    fn oracle_encoder_init_run(
        rust: bool,
        fail: i32,
        callbacks: u32,
        value: u32,
        replace: bool,
        out: *mut u32,
    );
    fn oracle_encoder_init_layout(out: *mut usize);
}

fn run(rust: bool, fail: i32, callbacks: u32, value: u32, replace: bool) -> [u32; 27] {
    // Keep the exported entry point live: the C bridge calls this symbol.
    let _entry = obs_encoder_init_steps;
    let mut out = [0; 27];
    // SAFETY: C uses a stack-owned real encoder layout and synchronous probes;
    // the output buffer holds exactly the documented 27 words.
    unsafe { oracle_encoder_init_run(rust, fail, callbacks, value, replace, out.as_mut_ptr()) };
    out
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 7];
    // SAFETY: the C probe writes seven size_t values to this array.
    unsafe { oracle_encoder_init_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, sentinel),
            offset_of!(Fields, context),
            offset_of!(Fields, mutex),
            offset_of!(Fields, defaults),
            offset_of!(Fields, defaults2)
        ]
    );
}

#[test]
fn every_stage_and_callback_table() {
    for fail in -1..=8 {
        for callbacks in 0..4 {
            for replace in [false, true] {
                for value in [0, 1, u32::MAX] {
                    let c = run(false, fail, callbacks, value, replace);
                    assert_eq!(run(true, fail, callbacks, value, replace), c);
                    assert_eq!(c[1], 1, "forward context and callback arguments");
                    assert_eq!(c[0], u32::from(!(0..=6).contains(&fail)));
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn randomized_original_body_parity(fail in -10i32..16, callbacks in any::<u32>(), value in any::<u32>(), replace in any::<bool>()) {
        prop_assert_eq!(run(true, fail, callbacks, value, replace), run(false, fail, callbacks, value, replace));
    }
}

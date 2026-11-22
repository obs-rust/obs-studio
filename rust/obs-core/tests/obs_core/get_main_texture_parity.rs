//! Original-C getter parity over the real adapters: a NULL main canvas (which
//! the C characterization cannot reach), no mix, an unrendered or rendered
//! mix, no texture, either canvas and either mix, with the state switched
//! between reads and no byte written. Excludes an uninitialized core, dead
//! canvases or mixes and a concurrent render or reset, matching C's
//! contract. No intentional differences. The real main canvas lifecycle
//! remains unchanged Tier 2 (test_get_main_texture).
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::get_main_texture::{Fields, obs_get_main_texture};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_get_main_texture_layout(layout: *mut usize);
    fn oracle_get_main_texture_check(
        states: *const i32,
        count: usize,
        fill: u8,
        getter: unsafe extern "C" fn() -> *mut c_void,
    ) -> bool;
    fn oracle_get_main_texture_expected(canvas: i32, mix: i32, rendered: i32, texture: i32) -> i32;
}

#[test]
fn real_c_adapter_layout() {
    let mut layout = [0; 6];
    // SAFETY: C writes exactly six layout values into this array.
    unsafe { oracle_get_main_texture_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, canvas),
            offset_of!(Fields, mix),
            offset_of!(Fields, rendered),
            offset_of!(Fields, texture),
        ]
    );
}

fn check(states: &[[i32; 4]], fill: u8) {
    let flat: Vec<i32> = states.iter().flatten().copied().collect();
    // SAFETY: C owns live real-layout core, canvas and mix fixtures, installs
    // a thread-local initialized core and never dereferences a texture token.
    assert!(unsafe {
        oracle_get_main_texture_check(flat.as_ptr(), states.len(), fill, obs_get_main_texture)
    });
}

/// Every state: no main canvas, either canvas with no mix or either mix, and
/// that mix unrendered or rendered with no texture or either token.
fn all_states() -> Vec<[i32; 4]> {
    let mut states = vec![[-1, -1, 0, -1]];
    for canvas in 0..2 {
        states.push([canvas, -1, 0, -1]);
        for mix in 0..2 {
            for rendered in 0..2 {
                for texture in -1..2 {
                    states.push([canvas, mix, rendered, texture]);
                }
            }
        }
    }
    states
}

#[test]
fn original_c_answers_are_the_characterized_ones() {
    for [canvas, mix, rendered, texture] in all_states() {
        // SAFETY: C builds and frees its own fixture.
        let answer = unsafe { oracle_get_main_texture_expected(canvas, mix, rendered, texture) };
        let expected = if canvas < 0 || mix < 0 || rendered == 0 {
            -1
        } else {
            texture
        };
        assert_eq!(
            answer, expected,
            "state {canvas} {mix} {rendered} {texture}"
        );
    }
}

#[test]
fn characterized_state_edges() {
    let states = all_states();
    for fill in [0, 1, 127, 128, 254, 255] {
        check(&states, fill);
        let reversed: Vec<_> = states.iter().rev().copied().collect();
        check(&reversed, fill);
    }
}

fn state() -> impl Strategy<Value = [i32; 4]> {
    (-1..2i32, -1..2i32, 0..2i32, -1..2i32).prop_map(|(c, m, r, t)| [c, m, r, t])
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn current_borrowed_texture_matches_original_c(
        states in proptest::collection::vec(state(), 1..12),
        fill in any::<u8>(),
    ) {
        check(&states, fill);
    }
}

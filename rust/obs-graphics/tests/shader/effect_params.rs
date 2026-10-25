//! Tier 1: the safe core of `libobs/graphics/effect-params.c.h`.
//!
//! The cases with the names of `test/cmocka/test_effect_param_values.c`
//! port its rules on the core, with the same inputs and expectations:
//! `changed` on new bytes and on a size change, the getter copies, the
//! color conversion, the next sampler. The rest of that file (each setter's
//! size and bytes, `gs_effect_set_default` without a default, the NULL log
//! lines) is the shim's, and the cmocka test itself runs against the shim in
//! the hybrid libobs (Tier 2); Tier 3 compares the shim with the C.
//!
//! The cases after those (`edge_*`) pin the edges of the safe API, and
//! proptests check the invariants.

use obs_graphics::effect_params::{
    ByteStore, GS_SHADER_PARAM_TEXTURE, bgra_to_vec4, copy_out, set_val, takes_next_sampler,
};
use proptest::prelude::*;

/// `DARRAY(uint8_t)` as a [`Vec`].
#[derive(Debug, Default, Clone)]
struct Store(Vec<u8>);

impl ByteStore for Store {
    fn bytes(&self) -> &[u8] {
        &self.0
    }
    fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
    fn resize(&mut self, size: usize) {
        self.0.resize(size, 0);
    }
}

#[test]
fn float_changes_only_on_new_bytes() {
    let mut cur = Store::default();
    let mut changed = false;

    set_val(&mut cur, &mut changed, &1.5f32.to_ne_bytes());
    assert_eq!(cur.0, 1.5f32.to_ne_bytes());
    assert!(changed);

    // The same bytes again: not changed.
    changed = false;
    set_val(&mut cur, &mut changed, &1.5f32.to_ne_bytes());
    assert!(!changed);

    // Different bytes: changed.
    set_val(&mut cur, &mut changed, &2.0f32.to_ne_bytes());
    assert_eq!(cur.0, 2.0f32.to_ne_bytes());
    assert!(changed);
}

#[test]
fn size_change_always_marks_changed() {
    let mut cur = Store::default();
    let mut changed = false;
    let vec2: Vec<u8> = [1.0f32, 2.0].iter().flat_map(|f| f.to_ne_bytes()).collect();
    set_val(&mut cur, &mut changed, &vec2);
    assert_eq!(cur.0, vec2);

    // The first 4 bytes are the same, the size is not: changed.
    changed = false;
    set_val(&mut cur, &mut changed, &1.0f32.to_ne_bytes());
    assert_eq!(cur.0, 1.0f32.to_ne_bytes());
    assert!(changed);

    // A size of 0: changed, nothing stored.
    changed = false;
    set_val(&mut cur, &mut changed, &[]);
    assert!(cur.0.is_empty());
    assert!(changed);
}

#[test]
fn texture_and_color() {
    // 0xAARRGGBB: 0x80 alpha, 0xFF red, 0x40 green, 0x20 blue.
    let v = bgra_to_vec4(0x80FF_4020);
    assert_eq!(
        v,
        [255.0 / 255.0, 64.0 / 255.0, 32.0 / 255.0, 128.0 / 255.0]
    );
}

#[test]
fn getters_copy_the_stored_bytes() {
    let src = [1u8, 2, 3, 4];
    let mut dst = [0u8; 4];
    assert_eq!(copy_out(&src, &mut dst), 4);
    assert_eq!(dst, src);

    // At most the stored size.
    let mut big = [9u8; 6];
    assert_eq!(copy_out(&src, &mut big), 4);
    assert_eq!(big, [1, 2, 3, 4, 9, 9]);
}

#[test]
fn next_sampler_only_on_textures() {
    assert!(takes_next_sampler(GS_SHADER_PARAM_TEXTURE));
    for t in 0..GS_SHADER_PARAM_TEXTURE {
        assert!(!takes_next_sampler(t));
    }
}

#[test]
fn edge_copy_out_into_a_smaller_buffer() {
    let mut dst = [0u8; 2];
    assert_eq!(copy_out(&[1, 2, 3, 4], &mut dst), 2);
    assert_eq!(dst, [1, 2]);
}

#[test]
fn edge_color_extremes() {
    assert_eq!(bgra_to_vec4(0), [0.0; 4]);
    assert_eq!(bgra_to_vec4(u32::MAX), [1.0; 4]);
    // Each channel lands in its own lane.
    assert_eq!(bgra_to_vec4(0x00FF_0000), [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(bgra_to_vec4(0x0000_FF00), [0.0, 1.0, 0.0, 0.0]);
    assert_eq!(bgra_to_vec4(0x0000_00FF), [0.0, 0.0, 1.0, 0.0]);
    assert_eq!(bgra_to_vec4(0xFF00_0000), [0.0, 0.0, 0.0, 1.0]);
}

proptest! {
    /// After set_val the store holds the data; changed is set exactly when
    /// the size or the bytes differed, and never cleared.
    #[test]
    fn prop_set_val(
        old in prop::collection::vec(any::<u8>(), 0..80),
        new in prop::collection::vec(any::<u8>(), 0..80),
        was_changed in any::<bool>(),
    ) {
        let mut cur = Store(old.clone());
        let mut changed = was_changed;
        set_val(&mut cur, &mut changed, &new);
        prop_assert_eq!(&cur.0, &new);
        prop_assert_eq!(changed, was_changed || old != new);
    }

    /// copy_out copies min(src, dst) bytes and leaves the rest of dst.
    #[test]
    fn prop_copy_out(
        src in prop::collection::vec(any::<u8>(), 0..64),
        dst0 in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        let mut dst = dst0.clone();
        let n = copy_out(&src, &mut dst);
        prop_assert_eq!(n, src.len().min(dst0.len()));
        prop_assert_eq!(&dst[..n], &src[..n]);
        prop_assert_eq!(&dst[n..], &dst0[n..]);
    }

    /// Every channel is its byte / 255.
    #[test]
    fn prop_bgra(bgra in any::<u32>()) {
        let v = bgra_to_vec4(bgra);
        let b = bgra.to_ne_bytes();
        prop_assert_eq!(v, [b[2], b[1], b[0], b[3]].map(|u| f32::from(u) / 255.0));
    }
}

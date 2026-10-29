//! Tier 1: the safe core of `libobs/obs-module-register-copy.c.h`.
//!
//! The first eighteen tests mirror the cases of
//! `test/cmocka/test_module_register_copy_steps.c` one to one, in the same
//! order, with the same sizes and expectations. The first eight call the
//! rules directly, as the C test calls the moved functions. The last ten
//! make the decisions `obs_register_source_s()`, `obs_register_output_s()`,
//! `obs_register_encoder_s()` and `obs_register_service_s()` make for the
//! C test's registrations: whether each callback lies within the size
//! (`HAS_VAL()`), how much is copied, whether the type data is freed on an
//! error, the size gate and the transition rule. The field offsets are
//! those of the four structures on the 64-bit targets libobs builds for.
//!
//! Cases marked "characterized, not endorsed" are odd but real (#2860)
//! and kept as the C has them. The tests after those pin the edges of each
//! function, and proptests cover any size, pointer size and field list.

use obs_core::module_register_copy::{
    Gate, SOURCE_TYPE_TRANSITION, U32_SIZE, gate, ignores_size, whole_size, within,
};
use proptest::prelude::*;

const PTR: usize = 8;

/// `OBS_SOURCE_TYPE_INPUT`, `_FILTER`, `_SCENE`.
const SOURCE_TYPE_INPUT: u32 = 0;
const SOURCE_TYPE_FILTER: u32 = 1;
const SOURCE_TYPE_SCENE: u32 = 3;

/// `struct obs_source_info` on a 64-bit build.
mod source {
    pub const TYPE: usize = 8;
    pub const OUTPUT_FLAGS: usize = 12;
    pub const GET_NAME: usize = 16;
    pub const GET_DEFAULTS: usize = 56;
    pub const TYPE_DATA: usize = 216;
    pub const FREE_TYPE_DATA: usize = 224;
    pub const ICON_TYPE: usize = 288;
    pub const MEDIA_PLAY_PAUSE: usize = 296;
    pub const VERSION: usize = 368;
    pub const SIZE: usize = 424;
    pub const U32: [usize; 4] = [TYPE, OUTPUT_FLAGS, ICON_TYPE, VERSION];
}

/// `struct obs_output_info` on a 64-bit build.
mod output {
    pub const FLAGS: usize = 8;
    pub const GET_NAME: usize = 16;
    pub const STOP: usize = 48;
    pub const GET_DEFAULTS: usize = 88;
    pub const TYPE_DATA: usize = 128;
    pub const FREE_TYPE_DATA: usize = 136;
    pub const SIZE: usize = 192;
    pub const U32: [usize; 1] = [FLAGS];
}

/// `struct obs_encoder_info` on a 64-bit build.
mod encoder {
    pub const TYPE: usize = 8;
    pub const GET_NAME: usize = 24;
    pub const ENCODE: usize = 48;
    pub const GET_DEFAULTS: usize = 64;
    pub const TYPE_DATA: usize = 120;
    pub const FREE_TYPE_DATA: usize = 128;
    pub const CAPS: usize = 136;
    pub const SIZE: usize = 192;
    pub const U32: [usize; 2] = [TYPE, CAPS];
}

/// `struct obs_service_info` on a 64-bit build.
mod service {
    pub const GET_NAME: usize = 8;
    pub const GET_DEFAULTS: usize = 56;
    pub const TYPE_DATA: usize = 128;
    pub const FREE_TYPE_DATA: usize = 136;
    pub const GET_PROTOCOL: usize = 184;
    pub const SIZE: usize = 216;
}

/// A pointer field lies within the size (`HAS_VAL()`).
fn has(offset: usize, size: usize) -> bool {
    within(offset, PTR, size)
}

/// A pointer field is copied whole.
fn copied(offset: usize, size: usize, u32_fields: &[usize]) -> bool {
    offset + PTR <= whole_size(size, PTR, u32_fields)
}

/// `HANDLE_ERROR()`: the type data is freed (both set) only when
/// `type_data` and `free_type_data` lie within the size.
fn frees(type_data: usize, free_type_data: usize, size: usize) -> bool {
    within(type_data, PTR, size) && within(free_type_data, PTR, size)
}

// ------------------------------------------------------------------ steps

/// Mirrors `test_whole_size_pointers_only`.
#[test]
fn test_whole_size_pointers_only() {
    for size in 0..64 {
        assert_eq!(whole_size(size, 8, &[]), size - size % 8);
        assert_eq!(whole_size(size, 4, &[]), size - size % 4);
    }
    assert_eq!(whole_size(0, 8, &[]), 0);
    assert_eq!(whole_size(7, 8, &[]), 0);
    assert_eq!(whole_size(8, 8, &[]), 8);
    assert_eq!(whole_size(15, 8, &[]), 8);
    assert_eq!(whole_size(16, 8, &[]), 16);
    assert_eq!(whole_size(3, 4, &[]), 0);
    assert_eq!(whole_size(5, 4, &[]), 4);
}

/// Mirrors `test_whole_size_source_head`.
#[test]
fn test_whole_size_source_head() {
    let u32 = [8, 12];
    assert_eq!(whole_size(8, 8, &u32), 8);
    assert_eq!(whole_size(9, 8, &u32), 8);
    assert_eq!(whole_size(11, 8, &u32), 8);
    assert_eq!(whole_size(12, 8, &u32), 12);
    assert_eq!(whole_size(13, 8, &u32), 12);
    assert_eq!(whole_size(15, 8, &u32), 12);
    assert_eq!(whole_size(16, 8, &u32), 16);
    assert_eq!(whole_size(23, 8, &u32), 16);
    assert_eq!(whole_size(24, 8, &u32), 24);
}

/// Mirrors `test_whole_size_padded_field`.
#[test]
fn test_whole_size_padded_field() {
    let u32 = [288];
    assert_eq!(whole_size(288, 8, &u32), 288);
    assert_eq!(whole_size(289, 8, &u32), 288);
    assert_eq!(whole_size(291, 8, &u32), 288);
    assert_eq!(whole_size(292, 8, &u32), 292);
    assert_eq!(whole_size(295, 8, &u32), 292);
    assert_eq!(whole_size(296, 8, &u32), 296);
    assert_eq!(whole_size(299, 8, &u32), 296);
    assert_eq!(whole_size(303, 8, &u32), 296);
    assert_eq!(whole_size(304, 8, &u32), 304);
}

/// Mirrors `test_whole_size_32bit`.
#[test]
fn test_whole_size_32bit() {
    let u32 = [4, 8, 144, 184];
    for size in 0..200 {
        assert_eq!(whole_size(size, 4, &u32), size - size % 4);
    }
}

/// Mirrors `test_whole_size_order`.
#[test]
fn test_whole_size_order() {
    assert_eq!(whole_size(12, 8, &[0]), 8);
    assert_eq!(whole_size(19, 8, &[16, 20]), 16);
    assert_eq!(whole_size(20, 8, &[16, 20]), 20);
    assert_eq!(whole_size(23, 8, &[16, 20]), 20);
    assert_eq!(whole_size(20, 8, &[20, 16]), 20);
    assert_eq!(whole_size(23, 8, &[20, 16]), 20);
    assert_eq!(whole_size(24, 8, &[16, 20]), 24);
    // characterized, not endorsed: the list is read once, in order, so
    // with overlapping fields a field that starts before the copy's new end
    // no longer counts.
    assert_eq!(whole_size(22, 8, &[16, 18]), 20);
    assert_eq!(whole_size(22, 8, &[18, 16]), 22);
}

/// Mirrors `test_within`.
#[test]
fn test_within() {
    assert!(within(0, 0, 0));
    assert!(within(0, 8, 8));
    assert!(!within(0, 8, 7));
    assert!(within(16, 8, 24));
    assert!(!within(16, 8, 23));
    assert!(within(16, 8, 1000));
    assert!(within(368, 4, 372));
    assert!(!within(368, 4, 371));
    assert!(!within(368, 4, 0));
}

/// Mirrors `test_gate`.
#[test]
fn test_gate() {
    assert_eq!(gate(0, 192), Gate::OutsideLoad);
    assert_eq!(gate(0, 0), Gate::OutsideLoad);
    assert_eq!(gate(1, 192), Gate::Copy);
    assert_eq!(gate(191, 192), Gate::Copy);
    assert_eq!(gate(192, 192), Gate::Copy);
    assert_eq!(gate(193, 192), Gate::TooLarge);
    assert_eq!(gate(usize::MAX, 192), Gate::TooLarge);
    assert_eq!(gate(1, 0), Gate::TooLarge);
}

/// Mirrors `test_ignores_size`.
#[test]
fn test_ignores_size() {
    assert!(ignores_size(SOURCE_TYPE_TRANSITION));
    assert!(!ignores_size(SOURCE_TYPE_INPUT));
    assert!(!ignores_size(SOURCE_TYPE_FILTER));
    assert!(!ignores_size(SOURCE_TYPE_SCENE));
    assert!(!ignores_size(4));
    assert!(!ignores_size(u32::MAX));
}

// ---------------------------------------------------------------- callers

/// Mirrors `test_source_callback_at_size`: get_name counts when it ends
/// exactly at the size, not one byte short; type_data is not freed.
#[test]
fn test_source_callback_at_size() {
    let end = source::GET_NAME + PTR;
    assert!(!has(source::GET_NAME, end - 1));
    assert!(!frees(source::TYPE_DATA, source::FREE_TYPE_DATA, end - 1));
    assert!(has(source::GET_NAME, end));
    assert!(copied(source::GET_NAME, end, &source::U32));
    assert!(!copied(source::GET_DEFAULTS, end, &source::U32));
}

/// Mirrors `test_source_pointer_cut`.
#[test]
fn test_source_pointer_cut() {
    let end = source::GET_DEFAULTS + PTR;
    assert!(!copied(source::GET_DEFAULTS, end - 1, &source::U32));
    assert!(!copied(source::GET_DEFAULTS, end - PTR + 1, &source::U32));
    assert!(copied(source::GET_DEFAULTS, end, &source::U32));
    for size in [end - 1, end - PTR + 1, end] {
        assert!(has(source::GET_NAME, size));
        assert_eq!(gate(size, source::SIZE), Gate::Copy);
    }
}

/// Mirrors `test_source_u32_field`: icon_type is dropped when the size
/// ends inside it and kept when the size ends inside the pointer after it.
#[test]
fn test_source_u32_field() {
    let end = source::ICON_TYPE + U32_SIZE;
    let next_end = source::MEDIA_PLAY_PAUSE + PTR;
    assert!(whole_size(end - 1, PTR, &source::U32) <= source::ICON_TYPE);
    assert!(whole_size(end, PTR, &source::U32) >= end);
    let next = whole_size(next_end - 1, PTR, &source::U32);
    assert!(next >= end);
    assert!(next < next_end);
    assert!(copied(source::GET_DEFAULTS, next_end - 1, &source::U32));
}

/// Mirrors `test_source_error_free`.
#[test]
fn test_source_error_free() {
    let end = source::FREE_TYPE_DATA + PTR;
    assert!(!frees(source::TYPE_DATA, source::FREE_TYPE_DATA, end - 1));
    assert!(frees(source::TYPE_DATA, source::FREE_TYPE_DATA, end));
}

/// Mirrors `test_source_size_zero`: nothing is copied, so get_name is
/// missing; nothing is freed. Characterized, not endorsed.
#[test]
fn test_source_size_zero() {
    assert_eq!(whole_size(0, PTR, &source::U32), 0);
    assert!(!has(source::GET_NAME, 0));
    assert!(!frees(source::TYPE_DATA, source::FREE_TYPE_DATA, 0));
}

/// Mirrors `test_source_too_large`: the source path checks
/// `size_supported()` itself; the free uses the in-size rule.
#[test]
fn test_source_too_large() {
    let size = source::SIZE + 8;
    assert_eq!(gate(size, source::SIZE), Gate::TooLarge);
    assert!(frees(source::TYPE_DATA, source::FREE_TYPE_DATA, size));
}

/// Mirrors `test_source_transition_sizes`: a transition's sizes are
/// ignored, an input's are not.
#[test]
fn test_source_transition_sizes() {
    assert!(ignores_size(SOURCE_TYPE_TRANSITION));
    assert!(!ignores_size(SOURCE_TYPE_INPUT));
}

/// Mirrors `test_output_sizes`.
#[test]
fn test_output_sizes() {
    let stop_end = output::STOP + PTR;
    let defaults_end = output::GET_DEFAULTS + PTR;
    let free_end = output::FREE_TYPE_DATA + PTR;

    assert!(!has(output::STOP, stop_end - 1));
    assert!(has(output::STOP, stop_end));
    assert_eq!(gate(stop_end, output::SIZE), Gate::Copy);
    assert!(!copied(output::GET_DEFAULTS, stop_end, &output::U32));

    assert!(!copied(
        output::GET_DEFAULTS,
        defaults_end - 1,
        &output::U32
    ));
    assert!(copied(output::GET_DEFAULTS, defaults_end, &output::U32));

    assert!(!frees(
        output::TYPE_DATA,
        output::FREE_TYPE_DATA,
        free_end - 1
    ));
    assert!(frees(output::TYPE_DATA, output::FREE_TYPE_DATA, free_end));

    // Characterized, not endorsed: a size of 0 fails the callback check
    // before the gate would report it.
    assert!(!has(output::GET_NAME, 0));
    assert_eq!(gate(0, output::SIZE), Gate::OutsideLoad);
    assert!(!frees(output::TYPE_DATA, output::FREE_TYPE_DATA, 0));

    let large = output::SIZE + 8;
    assert_eq!(gate(large, output::SIZE), Gate::TooLarge);
    assert!(frees(output::TYPE_DATA, output::FREE_TYPE_DATA, large));
}

/// Mirrors `test_encoder_sizes`.
#[test]
fn test_encoder_sizes() {
    let encode_end = encoder::ENCODE + PTR;
    let defaults_end = encoder::GET_DEFAULTS + PTR;
    let free_end = encoder::FREE_TYPE_DATA + PTR;

    assert!(!has(encoder::ENCODE, encode_end - 1));
    assert!(!within(encoder::CAPS, U32_SIZE, encode_end - 1));
    assert!(has(encoder::ENCODE, encode_end));
    assert_eq!(gate(encode_end, encoder::SIZE), Gate::Copy);
    assert!(!copied(encoder::GET_DEFAULTS, encode_end, &encoder::U32));

    assert!(!copied(
        encoder::GET_DEFAULTS,
        defaults_end - 1,
        &encoder::U32
    ));
    assert!(copied(encoder::GET_DEFAULTS, defaults_end, &encoder::U32));

    assert!(!frees(
        encoder::TYPE_DATA,
        encoder::FREE_TYPE_DATA,
        free_end - 1
    ));
    assert!(frees(encoder::TYPE_DATA, encoder::FREE_TYPE_DATA, free_end));

    // Characterized, not endorsed: the callback check fails first.
    assert!(!has(encoder::GET_NAME, 0));
    assert!(!frees(encoder::TYPE_DATA, encoder::FREE_TYPE_DATA, 0));

    let large = encoder::SIZE + 8;
    assert_eq!(gate(large, encoder::SIZE), Gate::TooLarge);
    assert!(frees(encoder::TYPE_DATA, encoder::FREE_TYPE_DATA, large));
}

/// Mirrors `test_service_sizes`.
#[test]
fn test_service_sizes() {
    let protocol_end = service::GET_PROTOCOL + PTR;
    let defaults_end = service::GET_DEFAULTS + PTR;
    let free_end = service::FREE_TYPE_DATA + PTR;

    assert!(!has(service::GET_PROTOCOL, protocol_end - 1));
    assert!(frees(
        service::TYPE_DATA,
        service::FREE_TYPE_DATA,
        protocol_end - 1
    ));
    assert!(has(service::GET_PROTOCOL, protocol_end));
    assert!(copied(service::GET_DEFAULTS, protocol_end, &[]));

    assert!(!has(service::GET_PROTOCOL, defaults_end - 1));
    assert!(!frees(
        service::TYPE_DATA,
        service::FREE_TYPE_DATA,
        defaults_end - 1
    ));

    assert!(!frees(
        service::TYPE_DATA,
        service::FREE_TYPE_DATA,
        free_end - 1
    ));
    assert!(frees(service::TYPE_DATA, service::FREE_TYPE_DATA, free_end));

    // Characterized, not endorsed: the callback check fails first.
    assert!(!has(service::GET_NAME, 0));
    assert!(!frees(service::TYPE_DATA, service::FREE_TYPE_DATA, 0));

    let large = service::SIZE + 8;
    assert_eq!(gate(large, service::SIZE), Gate::TooLarge);
    assert!(frees(service::TYPE_DATA, service::FREE_TYPE_DATA, large));
}

// ------------------------------------------------------------------ edges

/// The 32-bit fields of each structure: the copy keeps each from its end,
/// and a whole structure is copied whole.
#[test]
fn whole_size_of_each_structure() {
    for (u32s, size) in [
        (&source::U32[..], source::SIZE),
        (&output::U32[..], output::SIZE),
        (&encoder::U32[..], encoder::SIZE),
        (&[][..], service::SIZE),
    ] {
        assert_eq!(whole_size(size, PTR, u32s), size);
        for &f in u32s {
            assert_eq!(whole_size(f + U32_SIZE, PTR, u32s), f + U32_SIZE);
            assert!(whole_size(f + U32_SIZE - 1, PTR, u32s) <= f);
        }
    }
    assert_eq!(
        whole_size(encoder::CAPS + U32_SIZE + 3, PTR, &encoder::U32),
        encoder::CAPS + U32_SIZE
    );
    assert_eq!(
        whole_size(source::VERSION + 7, PTR, &source::U32),
        source::VERSION + U32_SIZE
    );
}

/// Sizes at the top of the range neither overflow nor panic.
#[test]
fn edges_of_usize() {
    assert_eq!(whole_size(usize::MAX, 8, &[]), usize::MAX - 7);
    assert_eq!(whole_size(usize::MAX, 1, &[]), usize::MAX);
    assert_eq!(whole_size(usize::MAX, 8, &[usize::MAX - 7]), usize::MAX - 3);
    assert!(within(usize::MAX - 8, 8, usize::MAX));
    assert!(within(0, usize::MAX, usize::MAX));
    assert_eq!(gate(usize::MAX, usize::MAX), Gate::Copy);
}

/// A field past `usize::MAX - 4`, or a field end past `usize::MAX`, would
/// wrap in C; no layout has one (the offsets are `offsetof()` constants).
/// Here such a field is never within the size.
#[test]
fn field_end_past_usize_max_is_not_within() {
    assert!(!within(usize::MAX, 1, usize::MAX));
    assert!(!within(usize::MAX - 3, 8, usize::MAX));
    assert!(!within(1, usize::MAX, usize::MAX));
    assert_eq!(whole_size(16, 8, &[usize::MAX - 1]), 16);
}

/// A pointer size of 0 (C divides by zero; libobs always passes
/// `sizeof(void *)`): the size is not rounded.
#[test]
fn zero_pointer_size_does_not_panic() {
    assert_eq!(whole_size(13, 0, &[]), 13);
    assert_eq!(whole_size(13, 0, &[8]), 13);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// The copy never exceeds the size, never stops short of the size
    /// rounded down, and ends at a pointer boundary or a 32-bit field's
    /// end.
    #[test]
    fn whole_size_bounds(
        size in 0usize..1 << 20,
        ptr in prop_oneof![Just(4usize), Just(8usize)],
        fields in prop::collection::vec(0usize..1 << 20, 0..6),
    ) {
        let w = whole_size(size, ptr, &fields);
        prop_assert!(w <= size);
        prop_assert!(w >= size - size % ptr);
        prop_assert!(w == size - size % ptr || fields.iter().any(|&f| f + U32_SIZE == w));
    }

    /// `within` is the C comparison wherever the field's end fits.
    #[test]
    fn within_is_end_at_most_size(
        offset in 0usize..usize::MAX / 2,
        field_size in 0usize..usize::MAX / 2,
        size in any::<usize>(),
    ) {
        prop_assert_eq!(within(offset, field_size, size), offset + field_size <= size);
    }

    /// The gate for any size and limit.
    #[test]
    fn gate_any(size in any::<usize>(), supported in any::<usize>()) {
        let expected = if size == 0 {
            Gate::OutsideLoad
        } else if size > supported {
            Gate::TooLarge
        } else {
            Gate::Copy
        };
        prop_assert_eq!(gate(size, supported), expected);
    }

    /// Only the transition type ignores its sizes.
    #[test]
    fn ignores_size_any(source_type in any::<u32>()) {
        prop_assert_eq!(ignores_size(source_type), source_type == SOURCE_TYPE_TRANSITION);
    }
}

//! Tier 3: original extracted C and Rust use the same thread-local vector recorder.
//! No float patterns or signed texture units are excluded. Recording the boundary
//! avoids out-of-domain array access in C; live context behavior belongs to Tier 2.
use obs_c_oracle as _;
use obs_graphics::ffi::{
    immediate_arguments::{gs_texcoord, gs_vertex2v, gs_vertex3f},
    vec2::vec2,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_gs_vertex3f(x: f32, y: f32, z: f32);
    fn oracle_gs_texcoord(x: f32, y: f32, unit: i32);
    fn oracle_gs_vertex2v(value: *const vec2);
    fn oracle_immediate_scalars_reset();
    fn oracle_immediate_scalars_count() -> usize;
    fn oracle_immediate_scalars_word(index: usize) -> u32;
}

fn trace() -> Vec<u32> {
    // SAFETY: read only initialized words in this thread's bounded recorder.
    unsafe {
        (0..oracle_immediate_scalars_count())
            .map(|i| oracle_immediate_scalars_word(i))
            .collect()
    }
}

fn compare(sequence: &[(u8, [u32; 3], i32)]) {
    assert!(sequence.len() <= 64);
    // SAFETY: each sequence fits the thread-local recorder. Both implementations
    // receive aligned live vec2 inputs; callbacks copy synchronously and never
    // index a graphics array, so all signed units are valid for this fixture.
    unsafe {
        oracle_immediate_scalars_reset();
        for &(kind, bits, unit) in sequence {
            let [x, y, z] = bits.map(f32::from_bits);
            let input = vec2 { x, y };
            match kind {
                0 => oracle_gs_vertex3f(x, y, z),
                1 => oracle_gs_vertex2v(&input),
                2 => oracle_gs_texcoord(x, y, unit),
                _ => unreachable!(),
            }
            assert_eq!([input.x.to_bits(), input.y.to_bits()], [bits[0], bits[1]]);
        }
        let expected = trace();
        // Independently pin exactly one call, its order, all vector lanes, and
        // full-width unit forwarding (same bits as C's runtime uint32_t cast).
        let specified: Vec<_> = sequence
            .iter()
            .flat_map(|&(kind, [x, y, z], unit)| {
                if kind == 2 {
                    [3, x, y, u32::from_ne_bytes(unit.to_ne_bytes()), 0]
                } else {
                    [1, x, y, if kind == 0 { z } else { 0 }, 0]
                }
            })
            .collect();
        assert_eq!(expected, specified);
        oracle_immediate_scalars_reset();
        for &(kind, bits, unit) in sequence {
            let [x, y, z] = bits.map(f32::from_bits);
            let input = vec2 { x, y };
            match kind {
                0 => gs_vertex3f(x, y, z),
                1 => gs_vertex2v(&input),
                2 => gs_texcoord(x, y, unit),
                _ => unreachable!(),
            }
            assert_eq!([input.x.to_bits(), input.y.to_bits()], [bits[0], bits[1]]);
        }
        assert_eq!(trace(), expected);
    }
}

#[test]
fn test_immediate_arguments_exact_downstream_calls_match_c() {
    let components = [
        [0x00000000, 0x80000000, 0x3f800000],
        [0x80000000, 0x00000000, 0xbf800000],
        [0x7f800000, 0xff800000, 0x7fc12345],
        [0xffc54321, 0x7fc12345, 0x80000000],
        [0x00000001, 0x80000001, 0x7f7fffff],
        [0xff7fffff, 0x3fa00000, 0xc0200000],
    ];
    compare(&[]);
    for bits in components {
        for unit in [i32::MIN, -65537, -1, 0, 1, 7, 15, 16, 65537, i32::MAX] {
            compare(&[(0, bits, unit), (1, bits, unit), (2, bits, unit)]);
        }
    }
}

#[test]
fn additional_float_edges_and_repeated_calls_match_c() {
    let edges = [
        0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000, 0x3f800000, 0xbf800000, 0x7f7fffff,
        0xff7fffff, 0x7f800000, 0xff800000, 0x7fc12345, 0xffc54321, 0x7f800001,
    ];
    for x in edges {
        for y in edges {
            for z in edges {
                compare(&[
                    (0, [x, y, z], 0),
                    (2, [y, z, x], i32::MIN),
                    (1, [z, x, y], 0),
                    (0, [x, y, z], 0),
                    (2, [x, y, z], i32::MAX),
                ]);
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
    fn random_component_bits_and_sequences_match_c(
        sequence in prop::collection::vec((0u8..3, any::<[u32; 3]>(), any::<i32>()), 0..=64)
    ) {
        compare(&sequence);
    }
}

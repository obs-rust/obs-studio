//! Tier 3: exact component bits and callback trace of the original extracted C.
//! No intentional differences or excluded float bit patterns.
use obs_c_oracle as _;
use obs_graphics::ffi::immediate_scalars::{gs_normal3f, gs_vertex2f};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_gs_vertex2f(x: f32, y: f32);
    fn oracle_gs_normal3f(x: f32, y: f32, z: f32);
    fn oracle_immediate_scalars_reset();
    fn oracle_immediate_scalars_count() -> usize;
    fn oracle_immediate_scalars_word(index: usize) -> u32;
}

fn trace() -> Vec<u32> {
    // SAFETY: all calls use this thread's fixture. Only initialized trace words
    // are read, and no pointer escapes the C callbacks.
    unsafe {
        (0..oracle_immediate_scalars_count())
            .map(|i| oracle_immediate_scalars_word(i))
            .collect()
    }
}

fn compare(sequence: &[(bool, [u32; 3])]) {
    // SAFETY: sequences have at most 64 calls, each recording five words into
    // this thread's bounded fixture. Both implementations submit live vec3s.
    unsafe {
        oracle_immediate_scalars_reset();
        for &(normal, bits) in sequence {
            let [x, y, z] = bits.map(f32::from_bits);
            if normal {
                oracle_gs_normal3f(x, y, z);
            } else {
                oracle_gs_vertex2f(x, y);
            }
        }
        let expected = trace();
        // Independently pin one call per wrapper, its identity and all lanes.
        let specified: Vec<_> = sequence
            .iter()
            .flat_map(|&(normal, [x, y, z])| {
                [
                    if normal { 2 } else { 1 },
                    x,
                    y,
                    if normal { z } else { 0 },
                    0,
                ]
            })
            .collect();
        assert_eq!(expected, specified);
        oracle_immediate_scalars_reset();
        for &(normal, bits) in sequence {
            let [x, y, z] = bits.map(f32::from_bits);
            if normal {
                gs_normal3f(x, y, z);
            } else {
                gs_vertex2f(x, y);
            }
        }
        assert_eq!(trace(), expected);
    }
}

#[test]
fn edge_component_bits_and_callback_order_match_c() {
    let edges = [
        0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000, 0x3f800000, 0xbf800000, 0x7f7fffff,
        0xff7fffff, 0x7f800000, 0xff800000, 0x7fc12345, 0xffc54321, 0x7f800001,
    ];
    compare(&[]);
    for x in edges {
        for y in edges {
            for z in edges {
                compare(&[
                    (false, [x, y, z]),
                    (true, [x, y, z]),
                    (true, [z, x, y]),
                    (false, [y, z, x]),
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
        sequence in prop::collection::vec((any::<bool>(), any::<[u32; 3]>()), 0..=64)
    ) {
        compare(&sequence);
    }
}

//! Tier 1: the dispatched output clamp (#3414) gives the scalar loop's bits.

use obs_c_oracle as _;
use obs_media_io::audio_clamp::{clamp_samples, clamp_samples_scalar};
use proptest::prelude::*;

fn sample() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        -4.0f32..4.0,
        Just(f32::NAN),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        Just(-0.0f32),
        Just(1.0f32),
        Just(-1.0f32),
    ]
}

#[test]
fn clamp_known_values() {
    let mut buf = [
        f32::NAN,
        2.0,
        -2.0,
        0.5,
        -0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];
    clamp_samples(&mut buf);
    assert_eq!(
        buf.map(f32::to_bits),
        [0.0f32, 1.0, -1.0, 0.5, -0.0, 1.0, -1.0].map(f32::to_bits)
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn edge_dispatched_clamp_matches_the_scalar_bits(v in prop::collection::vec(sample(), 0..300)) {
        let mut a = v.clone();
        let mut b = v;
        clamp_samples(&mut a);
        clamp_samples_scalar(&mut b);
        prop_assert_eq!(a.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                        b.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
    }
}

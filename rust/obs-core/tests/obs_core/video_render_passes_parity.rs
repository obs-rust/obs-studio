//! Tier 3 compares C bodies and Rust ABI through the real production adapter.
//! Recording doubles establish orchestration parity, not GPU image quality.
use obs_c_oracle::video_render_passes::trace;
use proptest::prelude::*;

fn compare(convert: bool, index: usize, count: usize, size: [u32; 2], plane: usize) {
    let c = trace(false, convert, index, count, size, plane);
    assert_eq!(trace(true, convert, index, count, size, plane), c);
    let safe =
        super::video_render_passes::run(convert, index, count, size[0], size[1], 100 + plane);
    assert_eq!(safe.trim_start_matches('M'), c);
}

#[test]
fn characterized_edges() {
    for size in [[0, 0], [1920, 1080], [u32::MAX, 1], [1, u32::MAX]] {
        for plane in 0..3 {
            for index in 0..2 {
                for count in [0, 1, 2, 17] {
                    compare(false, index, count, size, plane);
                    compare(true, index, count, size, plane);
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
    fn arbitrary_render_traces(convert in any::<bool>(), index in 0usize..2, count in 0usize..32,
                               width in any::<u32>(), height in any::<u32>(), plane in 0usize..3) {
        compare(convert,index,count,[width,height],plane);
    }
}

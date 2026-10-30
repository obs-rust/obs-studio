//! Exhaustive rounding coverage through the production safe API.

use obs_graphics::vec2::Vec2;

fn ceil_f32(x: f32) -> f32 {
    Vec2::new(x, x).ceil().x
}
fn floor_f32(x: f32) -> f32 {
    Vec2::new(x, x).floor().x
}

fn rounded_nan(x: f32) -> f32 {
    let input = obs_c_oracle::vec2::OracleVec2 { x, y: x };
    let mut output = input;
    // SAFETY: both pointers refer to live, aligned vectors.
    unsafe {
        obs_c_oracle::vec2::oracle_vec2_ceil(&mut output, &input);
    }
    output.x
}

/// Bitwise equal, or both NaN with the input's bits (the C behaviour).
fn matches(got: f32, want: f32, x: f32, quiet_nan: bool) -> bool {
    if x.is_nan() {
        got.to_bits() == (x.to_bits() | if quiet_nan { 0x0040_0000 } else { 0 })
    } else {
        got.to_bits() == want.to_bits()
    }
}

/// Checks every `f32` bit pattern, split across threads. The crate's dev
/// profile is optimized (workspace `Cargo.toml`), so this takes seconds.
/// Below the `full` parity level (PR, queue, master push) it checks one
/// pattern in each window of ten; the nightly and release runs sweep all
/// 2^32 (#2888). `edge_values` keeps the special values on every run.
#[test]
fn exhaustive_matches_libm() {
    const CHUNKS: u64 = 64;
    let span = (1u64 << 32) / CHUNKS;
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..CHUNKS)
            .map(|c| {
                s.spawn(move || {
                    let quiet_nan =
                        rounded_nan(f32::from_bits(0x7f80_0001)).to_bits() == 0x7fc0_0001;
                    for bits in obs_c_oracle::parity::sweep(c * span..(c + 1) * span) {
                        #[allow(clippy::cast_possible_truncation)]
                        let x = f32::from_bits(bits as u32);
                        assert!(
                            matches(ceil_f32(x), x.ceil(), x, quiet_nan),
                            "ceil({bits:#010x}) = {:#010x}, want {:#010x}",
                            ceil_f32(x).to_bits(),
                            x.ceil().to_bits()
                        );
                        assert!(
                            matches(floor_f32(x), x.floor(), x, quiet_nan),
                            "floor({bits:#010x}) = {:#010x}, want {:#010x}",
                            floor_f32(x).to_bits(),
                            x.floor().to_bits()
                        );
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().expect("exhaustive chunk panicked");
        }
    });
}

#[test]
fn edge_values() {
    let cases: [(f32, f32, f32); 12] = [
        (0.0, 0.0, 0.0),
        (-0.0, -0.0, -0.0),
        (0.5, 1.0, 0.0),
        (-0.5, -0.0, -1.0),
        (1.0, 1.0, 1.0),
        (-1.5, -1.0, -2.0),
        (f32::from_bits(1), 1.0, 0.0),
        (-f32::from_bits(1), -0.0, -1.0),
        (8_388_607.5, 8_388_608.0, 8_388_607.0),
        (2_147_483_648.0, 2_147_483_648.0, 2_147_483_648.0),
        (f32::INFINITY, f32::INFINITY, f32::INFINITY),
        (f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
    ];
    for (x, c, f) in cases {
        assert_eq!(ceil_f32(x).to_bits(), c.to_bits(), "ceil({x})");
        assert_eq!(floor_f32(x).to_bits(), f.to_bits(), "floor({x})");
    }
    let snan = f32::from_bits(0x7f80_0001);
    assert_eq!(ceil_f32(snan).to_bits(), rounded_nan(snan).to_bits());
    assert_eq!(floor_f32(snan).to_bits(), rounded_nan(snan).to_bits());
}

//! Mirrors the output getter characterization and congestion clamp C tests.
use obs_output::output_congestion::get;
use std::cell::Cell;

#[test]
fn mirrors_absent_callback_zero() {
    assert_eq!(get(Some(()), |_| None, || panic!("valid output")).to_bits(), 0);
}

#[test]
fn null_logs_once_without_callback() {
    let logs = Cell::new(0);
    assert_eq!(get(None::<()>, |_| panic!("NULL callback"), || logs.set(logs.get() + 1)).to_bits(), 0);
    assert_eq!(logs.get(), 1);
}

#[test]
fn mirrors_clamp_edges_and_preserves_callback_data() {
    for (bits, expected) in [
        (0, 0), (0x8000_0000, 0x8000_0000),
        (0x3f00_0000, 0x3f00_0000), (0x3f80_0000, 0x3f80_0000),
        (0xbf80_0000, 0), (0x4000_0000, 0x3f80_0000),
        (0xff80_0000, 0), (0x7f80_0000, 0x3f80_0000),
        (0x7fc1_2345, 0x7fc1_2345), (0xffc1_2345, 0xffc1_2345),
    ] {
        let calls = Cell::new(0);
        let result = get(Some(71), |data| {
            assert_eq!(data, 71);
            calls.set(calls.get() + 1);
            Some(f32::from_bits(bits))
        }, || panic!("valid output"));
        assert_eq!(result.to_bits(), expected);
        assert_eq!(calls.get(), 1);
    }
}

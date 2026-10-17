//! Tier 1 tests for obs-rust/obs-studio#1026: macOS GPU utilization from
//! the `IOAccelerator` `PerformanceStatistics` dictionary. New code with no
//! C original, so Tiers 2 and 3 do not apply. The dictionary is captured
//! from an M1 Max (macOS 26.7.1, `ioreg -r -c IOAccelerator -d 1 -l`).

use obs_gpu_status::Utilization;
use obs_gpu_status::iokit::{DEVICE_UTILIZATION, device_utilization};

fn s(stats: &[(&str, i64)]) -> Vec<(String, i64)> {
    stats.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}

/// `AGXAcceleratorG13X`'s number entries while the GPU was busy.
fn capture() -> Vec<(String, i64)> {
    s(&[
        ("In use system memory (driver)", 0),
        ("Alloc system memory", 8_929_705_984),
        ("Tiler Utilization %", 24),
        ("recoveryCount", 0),
        ("lastRecoveryTime", 0),
        ("Renderer Utilization %", 23),
        ("TiledSceneBytes", 2_555_904),
        ("Device Utilization %", 25),
        ("SplitSceneCount", 0),
        ("Allocated PB Size", 118_620_160),
        ("In use system memory", 2_035_843_072),
    ])
}

#[test]
fn device_utilization_is_the_whole_gpu() {
    assert_eq!(device_utilization(&capture()), Utilization::Percent(25));
}

#[test]
fn the_bounds_are_percentages() {
    assert_eq!(
        device_utilization(&s(&[(DEVICE_UTILIZATION, 0)])),
        Utilization::Percent(0)
    );
    assert_eq!(
        device_utilization(&s(&[(DEVICE_UTILIZATION, 100)])),
        Utilization::Percent(100)
    );
}

#[test]
fn a_value_outside_0_to_100_is_unavailable() {
    for v in [101, -1, i64::MAX, i64::MIN] {
        assert_eq!(
            device_utilization(&s(&[(DEVICE_UTILIZATION, v)])),
            Utilization::Unavailable,
            "{v}"
        );
    }
}

#[test]
fn a_missing_key_is_unavailable() {
    let mut stats = capture();
    stats.retain(|(k, _)| k != DEVICE_UTILIZATION);
    // Renderer and Tiler are parts of the GPU, not a stand-in for the whole.
    assert_eq!(device_utilization(&stats), Utilization::Unavailable);
    assert_eq!(device_utilization(&[]), Utilization::Unavailable);
}

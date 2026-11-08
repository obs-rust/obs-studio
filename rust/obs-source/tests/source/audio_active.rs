//! Tier 1 mirrors test_source_audio_active's NULL and repeated live reads.
use core::sync::atomic::{AtomicBool, Ordering};
use obs_source::audio_active::active;

#[test]
fn null_defaults() {
    let mut logs = 0;
    assert!(!active(None, || logs += 1));
    assert_eq!(logs, 1);
}

#[test]
fn independent_fields() {
    let field = AtomicBool::new(false);
    for bits in 0..16 {
        field.store(bits & 1 != 0, Ordering::SeqCst);
        for _ in 0..2 {
            assert_eq!(active(Some(&field), || panic!("valid source logged")), bits & 1 != 0);
            assert_eq!(field.load(Ordering::SeqCst), bits & 1 != 0);
        }
    }
}

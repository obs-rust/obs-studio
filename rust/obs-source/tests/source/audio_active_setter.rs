//! Tier 1 mirrors test_source_audio_active_setter's NULL, state and signals.
use core::cell::Cell;
use core::sync::atomic::{AtomicBool, Ordering};
use obs_source::audio_active_setter::set_active;
use obs_source::monitoring::Active;

#[test]
fn null_is_logged() {
    let count = Cell::new(0);
    for active in [true, false] {
        set_active(None, active, || count.set(count.get() + 1), |_| panic!("NULL signal"));
    }
    assert_eq!(count.get(), 2);
}

#[test]
fn state_and_signals() {
    for private in [false, true] {
        for previous in [false, true] {
            for active in [false, true] {
                let field = AtomicBool::new(previous);
                let mut events = Vec::new();
                let identity = &field as *const _;
                for _ in 0..2 {
                    set_active(Some(&field), active, || panic!("valid source"), |signal| {
                        assert_eq!(field.load(Ordering::SeqCst), active);
                        assert_eq!(&field as *const _, identity);
                        let first = match signal {
                            Active::Activate => 1,
                            Active::Deactivate => 3,
                            Active::None => panic!("no empty signal"),
                        };
                        if !private { events.push(first); }
                        events.push(first + 1);
                    });
                    assert_eq!(field.load(Ordering::SeqCst), active);
                    let expected = if previous == active { vec![] } else if private {
                        vec![if active { 2 } else { 4 }]
                    } else { vec![if active { 1 } else { 3 }, if active { 2 } else { 4 }] };
                    assert_eq!(events, expected);
                }
            }
        }
    }
}

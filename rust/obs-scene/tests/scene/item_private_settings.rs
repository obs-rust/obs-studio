//! Tier 1 mirrors test_sceneitem_private_settings.c through the safe core.
use core::cell::Cell;
use obs_scene::item_private_settings::{NAME, get};

#[test]
fn null_item_logs_once_and_returns_default() {
    let logs = Cell::new(0);
    let result: usize = get(
        None,
        |_| panic!("a missing item takes no reference"),
        |name| {
            assert_eq!(name, NAME);
            assert_eq!(name, c"obs_sceneitem_get_private_settings");
            logs.set(logs.get() + 1);
        },
    );
    assert_eq!(result, 0);
    assert_eq!(logs.get(), 1);
}

/// Characterized, not endorsed: a stored NULL is returned unchanged, not
/// replaced or diagnosed, and still passed to addref (which accepts NULL).
#[test]
fn stored_null_is_returned_unchanged() {
    let refs = Cell::new(Vec::new());
    let result: usize = get(
        Some(0),
        |value| {
            let mut seen = refs.take();
            seen.push(value);
            refs.set(seen);
        },
        |_| panic!("a live item must not log"),
    );
    assert_eq!(result, 0);
    assert_eq!(refs.take(), vec![0]);
}

/// Each call returns the stored identity and adds exactly one reference.
#[test]
fn stored_identity_one_reference_per_call() {
    for stored in [1usize, 0x1000, usize::MAX] {
        for calls in [1usize, 2, 5] {
            let refs = Cell::new(0usize);
            for _ in 0..calls {
                let result = get(
                    Some(stored),
                    |value| {
                        assert_eq!(value, stored);
                        refs.set(refs.get() + 1);
                    },
                    |_| panic!("a live item must not log"),
                );
                assert_eq!(result, stored);
            }
            assert_eq!(refs.get(), calls);
        }
    }
}

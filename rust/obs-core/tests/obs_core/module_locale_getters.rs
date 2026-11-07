//! Tier 1 mirrors test_module_locale_getters.c using safe callback fixtures.
use obs_core::module_locale_getters::{get_string, get_text};
use std::cell::Cell;

#[test]
fn locale_lookup_missing_callback_and_lookup() {
    assert!(!get_string(None::<fn() -> bool>));
    assert_eq!(get_text("Key", None::<fn(&mut &str) -> bool>), "Key");
    assert_eq!(
        get_text(None::<&str>, None::<fn(&mut Option<&str>) -> bool>),
        None
    );
    let calls = Cell::new(0);
    let mut out = "sentinel";
    assert!(!get_string(Some(|| {
        calls.set(calls.get() + 1);
        false
    })));
    assert_eq!(out, "sentinel");
    assert_eq!(
        get_text(
            "Key",
            Some(|_: &mut &str| {
                calls.set(calls.get() + 1);
                false
            })
        ),
        "Key"
    );
    assert_eq!(calls.get(), 2);
    // NULL lookup and missing keys do not write; empty translations are missing.
    for key in [None, Some("Missing"), Some("Empty")] {
        assert!(!get_string(Some(|| false)));
        assert_eq!(out, "sentinel");
        assert_eq!(get_text(key, Some(|_: &mut Option<&str>| false)), key);
    }
    assert!(get_string(Some(|| {
        out = "Value";
        true
    })));
    assert_eq!(out, "Value");
    assert_eq!(
        get_text(
            "Key",
            Some(|value: &mut &str| {
                *value = "Value";
                true
            })
        ),
        "Value"
    );
}

#[test]
fn locale_lookup_keeps_writes_even_when_callback_returns_false() {
    let mut out = "sentinel";
    assert!(!get_string(Some(|| {
        out = "changed";
        false
    })));
    assert_eq!(out, "changed");
    assert_eq!(
        get_text(
            "Key",
            Some(|value: &mut &str| {
                *value = "changed";
                false
            })
        ),
        "changed"
    );
    assert_eq!(
        get_text(
            Some("Key"),
            Some(|value: &mut Option<&str>| {
                *value = None;
                false
            })
        ),
        None
    );
    // A callback decides which inputs and output pointers it accepts.
    assert!(!get_string(Some(|| false)));
}

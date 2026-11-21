//! Tier 1 mirrors test_output_caption_text1.c on the safe core: the NULL
//! diagnostic with nothing forwarded, and every live call forwarded once
//! with its own output and exactly two seconds. The queueing it forwards to
//! (inactive outputs, sparse tracks, pagination) is
//! `obs_output_output_caption_text2()`'s, still C, and Tier 2 runs it.
use obs_output::output_caption_text1::{DISPLAY_DURATION, caption_text1};
use std::cell::{Cell, RefCell};

#[test]
fn null_output_logs_once_and_forwards_nothing() {
    let logs = Cell::new(0);
    caption_text1(
        None::<u8>,
        || logs.set(logs.get() + 1),
        |_, _| panic!("a NULL output forwarded"),
    );
    assert_eq!(logs.get(), 1);
}

#[test]
fn live_output_forwards_once_with_two_seconds() {
    let calls = RefCell::new(Vec::new());
    caption_text1(
        Some("output"),
        || panic!("a live output logged"),
        |output, duration| calls.borrow_mut().push((output, duration.to_bits())),
    );
    assert_eq!(*calls.borrow(), [("output", 2.0_f64.to_bits())]);
}

#[test]
fn display_duration_is_the_c_float_literal() {
    // The C passes (double)2.0f.
    assert_eq!(DISPLAY_DURATION.to_bits(), f64::from(2.0_f32).to_bits());
}

#[test]
fn every_call_forwards_its_own_output() {
    let calls = RefCell::new(Vec::new());
    for output in [1, 2, 2] {
        caption_text1(
            Some(output),
            || panic!("a live output logged"),
            |output, duration| calls.borrow_mut().push((output, duration)),
        );
    }
    assert_eq!(
        *calls.borrow(),
        [
            (1, DISPLAY_DURATION),
            (2, DISPLAY_DURATION),
            (2, DISPLAY_DURATION)
        ]
    );
}

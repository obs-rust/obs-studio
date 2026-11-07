//! Tier 1: mirrors test_output_can_pause.c (#3895).
use obs_output::output_can_pause::{OBS_OUTPUT_CAN_PAUSE, can_pause};
use obs_output::NullLog;
use std::cell::RefCell;
use std::ffi::CStr;

#[derive(Default)]
struct Log(RefCell<Vec<&'static CStr>>);
impl NullLog for Log {
    fn null_output(&self, name: &'static CStr) {
        self.0.borrow_mut().push(name);
    }
}

#[test]
fn test_flags() {
    let log = Log::default();
    for flags in [0, OBS_OUTPUT_CAN_PAUSE, u32::MAX, u32::MAX ^ OBS_OUTPUT_CAN_PAUSE, 7] {
        assert_eq!(can_pause(Some(flags), &log), flags & OBS_OUTPUT_CAN_PAUSE != 0);
    }
    for bit in 0..32 {
        let flags = 1 << bit;
        assert_eq!(can_pause(Some(flags), &log), flags == OBS_OUTPUT_CAN_PAUSE);
        assert!(can_pause(Some(flags | OBS_OUTPUT_CAN_PAUSE), &log));
    }
    assert!(log.0.borrow().is_empty());
}

#[test]
fn test_null() {
    let log = Log::default();
    assert!(!can_pause(None, &log));
    assert_eq!(*log.0.borrow(), [c"obs_output_can_pause"]);
}

//! Mirrors test_encoder_output_getters_characterization.c (#4118).
use obs_output::{NullLog, output_frames_dropped::get};
use std::{cell::{Cell, RefCell}, ffi::CStr};

#[derive(Default)]
struct Log(RefCell<Vec<&'static CStr>>);
impl NullLog for Log {
    fn null_output(&self, name: &'static CStr) {
        self.0.borrow_mut().push(name);
    }
}

#[test]
fn mirrors_missing_callback_zero() {
    let log = Log::default();
    assert_eq!(get(Some(()), |_| None, &log), 0);
    assert!(log.0.borrow().is_empty());
}

#[test]
fn mirrors_null_zero_and_log_without_query() {
    let log = Log::default();
    assert_eq!(get(None::<()>, |_| panic!("NULL must not query"), &log), 0);
    assert_eq!(*log.0.borrow(), [c"obs_output_get_frames_dropped"]);
}

#[test]
fn signed_callback_result_and_context_once() {
    for result in [i32::MIN, -1, 0, 1, i32::MAX] {
        let log = Log::default();
        let calls = Cell::new(0);
        assert_eq!(get(Some(29), |context| {
            assert_eq!(context, 29);
            calls.set(calls.get() + 1);
            Some(result)
        }, &log), result);
        assert_eq!(calls.get(), 1);
        assert!(log.0.borrow().is_empty());
    }
}

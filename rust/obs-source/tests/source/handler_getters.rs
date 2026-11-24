//! Tier 1 mirrors test_source_handler_getters.c and
//! test_source_signal_handler_getter.c without FFI or unsafe code.
use obs_source::handler_getters::{proc, signal};

#[test]
fn borrowed_handlers() {
    for signal in [None, Some(7_u32)] {
        for handler in [None, Some(9_u32)] {
            let before = (signal, handler);
            for _ in 0..2 {
                assert_eq!(
                    proc(Some(handler), |_| panic!("valid source must not log")),
                    handler
                );
            }
            assert_eq!((signal, handler), before);
        }
    }
}

#[test]
fn null_source_logs_getter_name() {
    let mut logs = Vec::new();
    assert_eq!(proc::<Option<u32>>(None, |name| logs.push(name)), None);
    assert_eq!(logs, [c"obs_source_get_proc_handler"]);
}

#[test]
fn borrowed_signal_handler() {
    // The answer is the signal handler alone, whatever the procedure handler
    // holds (absent, another handler, or the signal handler itself).
    for handler in [None, Some(7_u32)] {
        for procs in [None, Some(9_u32), handler] {
            let before = (handler, procs);
            for _ in 0..2 {
                assert_eq!(
                    signal(Some(handler), |_| panic!("valid source must not log")),
                    handler
                );
            }
            assert_eq!((handler, procs), before);
        }
    }
}

#[test]
fn null_source_logs_signal_getter_name() {
    let mut logs = Vec::new();
    assert_eq!(signal::<Option<u32>>(None, |name| logs.push(name)), None);
    assert_eq!(logs, [c"obs_source_get_signal_handler"]);
}

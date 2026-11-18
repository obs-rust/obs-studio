//! Tier 1 mirrors test_service_handler_getters.c without FFI or unsafe code.
use obs_output::service_handler_getters::{Handlers, proc, signal};

#[test]
fn borrowed_handlers() {
    let values = [None, Some(7_u32), Some(9_u32)];
    for v in values {
        for a in values {
            let fields = Handlers { signal: v, proc: a };
            for _ in 0..2 {
                assert_eq!(
                    signal(Some(fields), |_| panic!("valid service must not log")),
                    v
                );
                assert_eq!(
                    proc(Some(fields), |_| panic!("valid service must not log")),
                    a
                );
            }
            assert_eq!(fields.signal, v);
            assert_eq!(fields.proc, a);
        }
    }
}

#[test]
fn null_service_logs_each_getter_name() {
    let mut logs = Vec::new();
    assert_eq!(signal::<Option<u32>>(None, |name| logs.push(name)), None);
    assert_eq!(proc::<Option<u32>>(None, |name| logs.push(name)), None);
    assert_eq!(
        logs,
        [
            c"obs_service_get_signal_handler",
            c"obs_service_get_proc_handler"
        ]
    );
}

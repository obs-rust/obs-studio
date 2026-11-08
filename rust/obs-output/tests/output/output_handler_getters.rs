//! Tier 1 mirrors test_output_handler_getters.c without FFI or unsafe code.
use obs_output::output_handler_getters::{Handlers, proc, signal};

#[test]
fn borrowed_handlers() {
    let values = [None, Some(7_u32)];
    for v in values {
        for a in values {
            let fields = Some(Handlers { signal: v, proc: a });
            for _ in 0..2 {
                assert_eq!(signal(fields, |_| panic!("valid output must not log")), v);
                assert_eq!(proc(fields, |_| panic!("valid output must not log")), a);
            }
        }
    }
}

#[test]
fn null_output_logs_each_getter_name() {
    let mut logs = Vec::new();
    assert_eq!(signal::<Option<u32>>(None, |name| logs.push(name)), None);
    assert_eq!(proc::<Option<u32>>(None, |name| logs.push(name)), None);
    assert_eq!(
        logs,
        [
            c"obs_output_get_signal_handler",
            c"obs_output_get_proc_handler"
        ]
    );
}

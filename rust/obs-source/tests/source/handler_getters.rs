//! Tier 1 mirrors test_source_handler_getters.c without FFI or unsafe code.
use obs_source::handler_getters::proc;

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

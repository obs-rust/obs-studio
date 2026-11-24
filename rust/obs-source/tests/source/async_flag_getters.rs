use obs_source::async_flag_getters::{decoupled, set_unbuffered, unbuffered};

#[test]
fn mirrors_c_null_defaults() {
    let mut logs = Vec::new();
    assert!(!unbuffered(None, |f, p| logs.push((f, p))));
    assert!(!decoupled(None, |f, p| logs.push((f, p))));
    assert_eq!(
        logs,
        [
            (c"obs_source_async_unbuffered", c"source"),
            (c"obs_source_async_decoupled", c"source"),
        ]
    );
}

#[test]
fn mirrors_c_independent_fields() {
    for u in [false, true] {
        for d in [false, true] {
            assert_eq!(unbuffered(Some(u), |_, _| panic!("unexpected log")), u);
            assert_eq!(decoupled(Some(d), |_, _| panic!("unexpected log")), d);
        }
    }
}

/// Tier 1 mirror of test_source_async_flag_pair.c's setter cases.
#[test]
fn set_unbuffered_null_logs_the_caller() {
    for value in [false, true] {
        let mut logs = Vec::new();
        set_unbuffered(None, value, |f, p| logs.push((f, p)));
        assert_eq!(logs, [(c"obs_source_set_async_unbuffered", c"source")]);
    }
}

#[test]
fn set_unbuffered_stores_only_the_flag() {
    for initial in [false, true] {
        // The neighbouring flag must keep its value through every store.
        let mut flags = [initial, !initial];
        for requested in [true, true, false, false, true, false] {
            set_unbuffered(Some(&mut flags[0]), requested, |_, _| {
                panic!("valid source logged")
            });
            assert_eq!(flags, [requested, !initial]);
            assert_eq!(
                unbuffered(Some(flags[0]), |_, _| panic!("unexpected log")),
                requested
            );
        }
    }
}

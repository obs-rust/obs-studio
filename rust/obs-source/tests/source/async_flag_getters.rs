use obs_source::async_flag_getters::{decoupled, last_async_ts, set_unbuffered, unbuffered};

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

/// Tier 1 mirror of test_source_last_async_ts.c (#5766): the stored timestamp
/// exactly, for edge values and every single bit.
#[test]
fn last_async_ts_answers_the_stored_timestamp() {
    let values = [
        0,
        1,
        33_333_333,
        1_000_000_000 * 60 * 60 * 24,
        0x8000_0000_0000_0000,
        0x7fff_ffff_ffff_ffff,
        0x0123_4567_89ab_cdef,
        u64::MAX,
    ];
    for value in values.into_iter().chain((0..64).map(|bit| 1u64 << bit)) {
        assert_eq!(last_async_ts(&value), value);
    }
}

#[test]
fn last_async_ts_writes_nothing() {
    for value in [0, 33_333_333, u64::MAX] {
        // The neighbouring timing fields hold other values and stay as they are.
        let fields = [!value, value, value ^ 0x5555_5555_5555_5555];
        for _ in 0..3 {
            assert_eq!(last_async_ts(&fields[1]), value);
            assert_eq!(fields, [!value, value, value ^ 0x5555_5555_5555_5555]);
        }
    }
}

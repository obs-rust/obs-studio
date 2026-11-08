use obs_output::output_pause_queries::{offset, paused};

#[test]
fn mirrors_c_null_defaults_and_diagnostics() {
    let mut logs = Vec::new();
    assert!(!paused(None, |name| logs.push(name)));
    assert_eq!(offset(None, |name| logs.push(name)), 0);
    assert_eq!(logs, [c"obs_output_paused", c"obs_output_get_pause_offset"]);
}

#[test]
fn mirrors_c_independent_states_without_mutation() {
    for p in [false, true] {
        for value in [0, 1, u32::MAX as u64, 1 << 63, u64::MAX] {
            let state = (p, value, 41, 73);
            assert_eq!(paused(Some(state.0), |_| panic!("unexpected log")), p);
            assert_eq!(offset(Some(state.1), |_| panic!("unexpected log")), value);
            assert_eq!(state, (p, value, 41, 73));
        }
    }
}

#[test]
fn mirrors_c_synchronized_offset() {
    let value = std::sync::Mutex::new(0);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            for i in 0..4096 {
                *value.lock().unwrap() = if i & 1 == 0 { 0 } else { u64::MAX };
            }
        });
        for _ in 0..4096 {
            let result = offset(Some(*value.lock().unwrap()), |_| panic!("unexpected log"));
            assert!(result == 0 || result == u64::MAX);
        }
    });
    assert!(value.try_lock().is_ok());
}

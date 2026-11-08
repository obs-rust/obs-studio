use obs_output::encoder_state_queries::{active, paused};

#[test]
fn mirrors_c_null_defaults_and_diagnostics() {
    let mut logs = Vec::new();
    assert!(!active(None, |name| logs.push(name)));
    assert!(!paused(None, |name| logs.push(name)));
    assert_eq!(logs, [c"obs_encoder_active", c"obs_encoder_paused"]);
}

#[test]
fn mirrors_c_independent_states_without_mutation() {
    for a in [false, true] {
        for p in [false, true] {
            let state = (a, p, true);
            assert_eq!(active(Some(state.0), |_| panic!("unexpected log")), a);
            assert_eq!(paused(Some(state.1), |_| panic!("unexpected log")), p);
            assert_eq!(state, (a, p, true));
        }
    }
}

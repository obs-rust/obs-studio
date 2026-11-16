//! Deterministic wait contract (#4447), using no sleeps or own threads.
use obs_mcp::server::ToolError;
use obs_mcp::tools::control::MediaState;
use obs_mcp::tools::waiting::{
    Condition, LastValue, Observation, ObserveInput, WaitBackend, WaitClock, WaitInput, run_wait,
};
use std::cell::Cell;
#[derive(Default)]
struct Clock(Cell<u64>);
impl WaitClock for Clock {
    fn now_ms(&self) -> u64 {
        self.0.get()
    }
    fn wait_ms(&self, ms: u64) {
        self.0.set(self.0.get() + ms);
    }
}
struct Backend<'a> {
    clock: &'a Clock,
    ready_at: u64,
    observations: Cell<u32>,
}
impl WaitBackend for Backend<'_> {
    fn observe(&self, _: &ObserveInput) -> Result<Observation, ToolError> {
        self.observations.set(self.observations.get() + 1);
        let ready = self.clock.now_ms() >= self.ready_at;
        Ok(Observation {
            satisfied: ready,
            value: LastValue::Media {
                state: Some(if ready {
                    MediaState::Playing
                } else {
                    MediaState::Opening
                }),
            },
        })
    }
}
fn input(timeout_ms: u64) -> WaitInput {
    WaitInput {
        condition: Condition::MediaState {
            name: "clip".into(),
            state: MediaState::Playing,
        },
        timeout_ms,
    }
}
#[test]
fn already_satisfied_zero_timeout_observes_without_waiting() {
    let clock = Clock::default();
    let backend = Backend {
        clock: &clock,
        ready_at: 0,
        observations: Cell::new(0),
    };
    let result = run_wait(&backend, input(0), &clock).unwrap();
    assert!(result.satisfied);
    assert_eq!(result.elapsed_ms, 0);
    assert_eq!(backend.observations.get(), 1);
}
#[test]
fn transition_and_timeout_preserve_last_observation() {
    let clock = Clock::default();
    let backend = Backend {
        clock: &clock,
        ready_at: 20,
        observations: Cell::new(0),
    };
    let result = run_wait(&backend, input(100), &clock).unwrap();
    assert!(result.satisfied);
    assert_eq!(result.elapsed_ms, 20);
    let clock = Clock::default();
    let backend = Backend {
        clock: &clock,
        ready_at: 100,
        observations: Cell::new(0),
    };
    let result = run_wait(&backend, input(15), &clock).unwrap();
    assert!(!result.satisfied);
    assert_eq!(result.elapsed_ms, 15);
    assert!(matches!(
        result.last_value,
        LastValue::Media {
            state: Some(MediaState::Opening)
        }
    ));
}

#[test]
fn cancellation_and_excessive_deadlines_do_not_touch_backend() {
    struct Cancelled;
    impl WaitBackend for Cancelled {
        fn cancelled(&self) -> bool {
            true
        }
        fn observe(&self, _: &ObserveInput) -> Result<Observation, ToolError> {
            panic!("cancelled host must not be queried")
        }
    }
    let clock = Clock::default();
    assert_eq!(
        run_wait(&Cancelled, input(100), &clock).unwrap_err().code,
        207
    );
    let backend = Backend {
        clock: &clock,
        ready_at: 0,
        observations: Cell::new(0),
    };
    assert_eq!(
        run_wait(&backend, input(300001), &clock).unwrap_err().code,
        604
    );
    assert_eq!(backend.observations.get(), 0);
}
#[test]
fn unmeasured_metrics_cannot_pass_a_threshold() {
    use obs_mcp::tools::waiting::metric_at_most;
    assert!(!metric_at_most(None, 1000.0));
    assert!(!metric_at_most(Some(f64::NAN), 1000.0));
    assert!(!metric_at_most(Some(1.0), f64::NAN));
    assert!(metric_at_most(Some(5.0), 5.0));
    assert!(!metric_at_most(Some(5.1), 5.0));
}

#[test]
fn scheduler_oversleep_cannot_turn_a_timeout_into_success() {
    struct Oversleep(Cell<u64>);
    impl WaitClock for Oversleep {
        fn now_ms(&self) -> u64 {
            self.0.get()
        }
        fn wait_ms(&self, ms: u64) {
            self.0.set(self.0.get() + ms + 10);
        }
    }
    struct Late<'a>(&'a Oversleep);
    impl WaitBackend for Late<'_> {
        fn observe(&self, _: &ObserveInput) -> Result<Observation, ToolError> {
            let active = self.0.now_ms() >= 15;
            Ok(Observation {
                satisfied: active,
                value: LastValue::Output {
                    active: Some(active),
                },
            })
        }
    }
    let clock = Oversleep(Cell::new(0));
    let result = run_wait(&Late(&clock), input(10), &clock).unwrap();
    assert!(!result.satisfied);
    assert_eq!(result.elapsed_ms, 20);
    assert!(matches!(
        result.last_value,
        LastValue::Output { active: Some(true) }
    ));
}

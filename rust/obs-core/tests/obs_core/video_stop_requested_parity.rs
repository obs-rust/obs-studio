//! Tier 3: the original C stop check and the Rust one through the private
//! ABI, over the same real core and the production bridge's accessors.
//! No intentional differences: the answer, every query (its video and
//! whether mixes_mutex was held) and the released mutex match exactly.
use super::video_stop_requested::{Model, expected_calls};
use core::ffi::c_void;
use obs_c_oracle::video_stop_requested::*;
use obs_core::ffi::video_stop_requested::obs_video_stop_requested;
use obs_core::video_stop_requested::stop_requested;
use proptest::prelude::*;
use std::cell::RefCell;

/// One `video_output_stopped()` call: the video asked and whether
/// `mixes_mutex` was held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Query {
    video: usize,
    locked: bool,
}

struct Run {
    core: *mut c_void,
    stopped: Vec<bool>,
    queries: Vec<Query>,
}
thread_local! { static RUN: RefCell<Option<Run>> = const { RefCell::new(None) }; }

unsafe extern "C" fn stopped(video: *mut c_void) -> bool {
    RUN.with_borrow_mut(|run| {
        let run = run.as_mut().expect("a run is active");
        // SAFETY: the run's core is live until check() frees it.
        let locked = unsafe { oracle_stop_requested_locked(run.core) };
        run.queries.push(Query {
            video: video as usize,
            locked,
        });
        let index = (0..run.stopped.len())
            // SAFETY: index < the core's mix count.
            .position(|i| unsafe { oracle_stop_requested_mix_video(run.core, i) } == video)
            .expect("the query names one of the core's mixes");
        run.stopped[index]
    })
}

// The bridge's accessors, wrapped to check that the count and every mix are
// read under mixes_mutex.
unsafe extern "C" fn lock(core: *mut c_void) {
    // SAFETY: the run's live core.
    unsafe { oracle_stop_requested_lock(core) }
}
unsafe extern "C" fn unlock(core: *mut c_void) {
    // SAFETY: the run's live core, locked by lock().
    unsafe { oracle_stop_requested_unlock(core) }
}
unsafe extern "C" fn count(core: *mut c_void) -> usize {
    // SAFETY: the run's live core.
    unsafe {
        assert!(oracle_stop_requested_locked(core), "num read unlocked");
        oracle_stop_requested_count(core)
    }
}
unsafe extern "C" fn video(core: *mut c_void, index: usize) -> *mut c_void {
    // SAFETY: the run's live core; the Rust passes index < count.
    unsafe {
        assert!(oracle_stop_requested_locked(core), "mix read unlocked");
        oracle_stop_requested_video(core, index)
    }
}

/// The answer and the queries, and that the mutex was released.
fn run(stop: &[bool], original: bool) -> (bool, Vec<Query>) {
    // SAFETY: freed below; a NULL core fails the assert first.
    let core = unsafe { oracle_stop_requested_new(stop.len()) };
    assert!(!core.is_null());
    RUN.with_borrow_mut(|run| {
        *run = Some(Run {
            core,
            stopped: stop.to_vec(),
            queries: Vec::new(),
        })
    });
    // SAFETY: a live core with stop.len() mixes, the bridge's accessors and a
    // non-unwinding query (an assert in a callback aborts the test).
    let answer = unsafe {
        if original {
            oracle_stop_requested_original(core, stopped)
        } else {
            obs_video_stop_requested(core, lock, unlock, count, video, stopped)
        }
    };
    let queries = RUN.with_borrow_mut(|run| run.take().expect("still active").queries);
    // SAFETY: the same live core.
    unsafe {
        assert!(!oracle_stop_requested_locked(core), "mixes_mutex released");
        oracle_stop_requested_free(core);
    }
    (answer, queries)
}

fn check(stop: &[bool]) {
    let expected = run(stop, true);
    assert_eq!(run(stop, false), expected);
    // The C asks every mix, in order, under the lock.
    assert_eq!(expected.1.len(), stop.len());
    assert!(expected.1.iter().all(|q| q.locked));
    let mut model = Model {
        stopped: stop.to_vec(),
        ..Model::default()
    };
    assert_eq!(stop_requested(&mut model), expected.0);
    assert_eq!(model.calls, expected_calls(stop.len()));
}

#[test]
fn every_vector_up_to_six_mixes() {
    for count in 0..=6usize {
        for mask in 0..(1u32 << count) {
            let stop: Vec<bool> = (0..count).map(|i| mask & (1 << i) != 0).collect();
            check(&stop);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn arbitrary_mix_lists(stop in proptest::collection::vec(any::<bool>(), 0..64)) {
        check(&stop);
    }
}

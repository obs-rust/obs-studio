//! Tier 1: mirrors the cases of test/cmocka/test_source_refs.c through the
//! safe API, on a recording host, plus what the cmocka test can't reach (a
//! control-less source, the exact step order of the final release).
// `c_long` is `i32` on Windows, `i64` elsewhere: widen it the same way on both.
#![allow(clippy::useless_conversion)]
use obs_source::strong_refs::{Host, addref, get_ref, release, try_get_ref, weak_get_source};
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

#[derive(Debug, PartialEq, Eq)]
enum Step {
    Alive,
    Warn,
    Control(usize),
    Source(usize),
    Destroy(usize),
    ReleaseWeak(usize),
}

/// Sources and their controls are indices; control `i` names source
/// `names[i]` and counts `refs[i]`.
struct Rec {
    alive: bool,
    controls: Vec<Option<usize>>,
    names: Vec<usize>,
    refs: Vec<CLongAtomic>,
    steps: Vec<Step>,
}

impl Rec {
    fn new(refs: &[i64]) -> Self {
        Self {
            alive: true,
            controls: (0..refs.len()).map(Some).collect(),
            names: (0..refs.len()).collect(),
            refs: refs.iter().map(|&r| CLongAtomic::new(r as _)).collect(),
            steps: Vec::new(),
        }
    }
    fn count(&self, i: usize) -> i64 {
        i64::from(self.refs[i].load(SeqCst))
    }
}

impl Host for Rec {
    type Source = usize;
    type Control = usize;
    fn core_alive(&mut self) -> bool {
        self.steps.push(Step::Alive);
        self.alive
    }
    fn warn_shutdown(&mut self) {
        self.steps.push(Step::Warn);
    }
    fn control(&mut self, source: usize) -> Option<usize> {
        self.steps.push(Step::Control(source));
        self.controls[source]
    }
    fn refs(&self, control: usize) -> &CLongAtomic {
        &self.refs[control]
    }
    fn source(&mut self, control: usize) -> usize {
        self.steps.push(Step::Source(control));
        self.names[control]
    }
    fn destroy(&mut self, source: usize) {
        self.steps.push(Step::Destroy(source));
    }
    fn release_weak(&mut self, control: usize) {
        self.steps.push(Step::ReleaseWeak(control));
    }
}

#[test]
fn null_inputs_do_nothing() {
    let mut h = Rec::new(&[0]);
    addref(None);
    release(&mut h, None);
    assert_eq!(get_ref(&mut h, None), None);
    assert_eq!(weak_get_source(&mut h, None), None);
    assert!(h.steps.is_empty());
    assert_eq!(h.count(0), 0);
}

#[test]
fn addref_and_release_move_only_refs() {
    let mut h = Rec::new(&[0]);
    addref(Some(h.refs(0)));
    assert_eq!(h.count(0), 1);
    addref(Some(h.refs(0)));
    assert_eq!(h.count(0), 2);
    release(&mut h, Some(0));
    assert_eq!(h.count(0), 1);
    release(&mut h, Some(0));
    assert_eq!(h.count(0), 0);
    assert_eq!(
        h.steps,
        [Step::Alive, Step::Control(0), Step::Alive, Step::Control(0)]
    );
}

#[test]
fn final_release_destroys_then_drops_its_weak_ref() {
    let mut h = Rec::new(&[0]);
    release(&mut h, Some(0));
    assert_eq!(h.count(0), -1);
    assert_eq!(
        h.steps,
        [
            Step::Alive,
            Step::Control(0),
            Step::Destroy(0),
            Step::ReleaseWeak(0)
        ]
    );
    h.steps.clear();
    assert_eq!(weak_get_source(&mut h, Some(0)), None);
    assert_eq!(get_ref(&mut h, Some(0)), None);
    assert_eq!(h.count(0), -1);
    assert_eq!(h.steps, [Step::Control(0)]);
}

/// Characterized, not endorsed: only the exact step from 0 to -1 destroys.
#[test]
fn below_zero_never_destroys() {
    let mut h = Rec::new(&[-1]);
    release(&mut h, Some(0));
    assert_eq!(h.count(0), -2);
    assert_eq!(h.steps, [Step::Alive, Step::Control(0)]);
}

#[test]
fn get_ref_of_a_live_source_adds_one() {
    let mut h = Rec::new(&[0, 0]);
    h.names[1] = 0;
    h.controls[0] = Some(1);
    assert_eq!(get_ref(&mut h, Some(0)), Some(0));
    assert_eq!((h.count(0), h.count(1)), (0, 1));
    assert_eq!(h.steps, [Step::Control(0), Step::Source(1)]);
}

#[test]
fn weak_promotion_of_a_live_source_adds_one() {
    let mut h = Rec::new(&[0]);
    assert_eq!(weak_get_source(&mut h, Some(0)), Some(0));
    assert_eq!(h.count(0), 1);
    assert_eq!(h.steps, [Step::Source(0)]);
}

#[test]
fn any_count_above_minus_one_is_promoted() {
    for start in [0, 1, 41, i64::from(i32::MAX) - 1] {
        let refs = CLongAtomic::new(start as _);
        assert!(try_get_ref(&refs));
        assert_eq!(i64::from(refs.load(SeqCst)), start + 1);
    }
    for start in [-1, -2, i64::from(i32::MIN)] {
        let refs = CLongAtomic::new(start as _);
        assert!(!try_get_ref(&refs));
        assert_eq!(i64::from(refs.load(SeqCst)), start);
    }
}

#[test]
fn a_dying_source_is_refused() {
    let mut h = Rec::new(&[-1]);
    assert_eq!(get_ref(&mut h, Some(0)), None);
    assert_eq!(weak_get_source(&mut h, Some(0)), None);
    assert_eq!(h.count(0), -1);
    assert_eq!(h.steps, [Step::Control(0)]);
}

#[test]
fn release_after_shutdown_warns_and_touches_nothing() {
    let mut h = Rec::new(&[0]);
    h.alive = false;
    release(&mut h, Some(0));
    assert_eq!(h.steps, [Step::Alive, Step::Warn]);
    assert_eq!(h.count(0), 0);
    h.steps.clear();
    release(&mut h, None);
    assert!(h.steps.is_empty());
}

/// A source without a control: get_ref refuses it, release drops nothing.
#[test]
fn a_control_less_source() {
    let mut h = Rec::new(&[0]);
    h.controls[0] = None;
    assert_eq!(get_ref(&mut h, Some(0)), None);
    release(&mut h, Some(0));
    assert_eq!(h.count(0), 0);
    assert_eq!(h.steps, [Step::Control(0), Step::Alive, Step::Control(0)]);
}

/// Promotions racing a release never resurrect a source at -1, and each
/// successful one is counted exactly once.
#[test]
fn concurrent_promotions_count_once() {
    let refs = CLongAtomic::new(0);
    let wins = std::thread::scope(|s| {
        let handles: Vec<_> = (0..8)
            .map(|_| s.spawn(|| (0..1000).filter(|_| try_get_ref(&refs)).count()))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .sum::<usize>()
    });
    assert_eq!(wins, 8000);
    assert_eq!(i64::from(refs.load(SeqCst)), 8000);
    refs.store(-1, SeqCst);
    assert!(!try_get_ref(&refs));
}

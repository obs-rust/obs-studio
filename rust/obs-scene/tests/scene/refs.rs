//! Tier 1 mirrors every assertion of test_scene_ref.c with safe lifetime effects.
use obs_core::{canvas_ref, object_ref};
use obs_scene::refs::{self, Host};
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

struct Fixture {
    strong: CLongAtomic,
    weak: CLongAtomic,
    reads: usize,
    acquisitions: usize,
    releases: usize,
    destroyed: usize,
    identity: Option<usize>,
    source: Option<usize>,
}
impl Fixture {
    fn new() -> Self {
        Self {
            strong: CLongAtomic::new(0),
            weak: CLongAtomic::new(1),
            reads: 0,
            acquisitions: 0,
            releases: 0,
            destroyed: 0,
            identity: None,
            source: Some(23),
        }
    }
}
impl Host for Fixture {
    type Scene = usize;
    type Source = Option<usize>;
    fn source(&mut self, scene: usize) -> Option<usize> {
        assert_eq!(scene, 17);
        self.reads += 1;
        self.source
    }
    fn acquire(&mut self, source: Option<usize>) -> bool {
        self.acquisitions += 1;
        // Reuse the production object-ref atomic core; source identity differs
        // from scene identity, and even an alternate promoted source is success.
        object_ref::get_ref(source.map(|source| (&self.strong, source + 1))).is_some()
    }
    fn release(&mut self, source: Option<usize>) {
        self.releases += 1;
        if source.is_some() && canvas_ref::release(Some(&self.strong)) {
            self.destroyed += 1;
            self.identity = source;
            assert!(!canvas_ref::release(Some(&self.weak)));
        }
    }
}

#[test]
fn test_scene_ref_null_before_during_and_after_core() {
    let mut f = Fixture::new();
    for _phase in 0..3 {
        assert_eq!(refs::get_ref(&mut f, None), None);
        refs::release(&mut f, None);
    }
    assert_eq!((f.reads, f.acquisitions, f.releases), (0, 0, 0));
    assert_eq!(f.strong.load(SeqCst), 0);
    assert_eq!(f.destroyed, 0);
}

fn exercise_refs(acquisitions: i32) {
    let mut f = Fixture::new();
    for i in 0..acquisitions {
        assert_eq!(refs::get_ref(&mut f, Some(17)), Some(17));
        assert_eq!(f.source, Some(23));
        assert_eq!(i128::from(f.strong.load(SeqCst)), i128::from(i + 1));
    }
    for i in 0..acquisitions {
        refs::release(&mut f, Some(17));
        assert!(f.strong.load(SeqCst) >= 0);
        assert_eq!(f.destroyed, 0);
        let retained = object_ref::get_ref(Some((&f.strong, 23)));
        assert_eq!(retained, f.source);
        assert_eq!(retained.map(|_| 17), Some(17));
        assert!(!canvas_ref::release(Some(&f.strong)));
        assert_eq!(
            i128::from(f.strong.load(SeqCst)),
            i128::from(acquisitions - i - 1)
        );
    }
    refs::release(&mut f, Some(17));
    assert_eq!(f.strong.load(SeqCst), -1);
    assert_eq!(refs::get_ref(&mut f, Some(17)), None);
    assert_eq!(object_ref::get_ref(Some((&f.strong, 23))), None);
    assert_eq!(f.destroyed, 1);
    assert_eq!(f.identity, Some(23));
    assert_eq!(f.source, Some(23));
    assert_eq!(f.acquisitions, usize::try_from(acquisitions + 1).unwrap());
    assert_eq!(f.releases, usize::try_from(acquisitions + 1).unwrap());
    assert_eq!(f.reads, f.acquisitions + f.releases);
    assert!(canvas_ref::release(Some(&f.weak)));
}

#[test]
fn test_scene_ref_last_reference() {
    exercise_refs(0);
}
#[test]
fn test_scene_ref_retained_reference() {
    exercise_refs(1);
}
#[test]
fn test_scene_ref_repeated_references() {
    for acquisitions in [2, 7, 32] {
        exercise_refs(acquisitions);
    }
}
#[test]
fn absent_source_and_expired_promotion_do_not_retain() {
    let mut f = Fixture::new();
    f.source = None;
    assert_eq!(refs::get_ref(&mut f, Some(17)), None);
    refs::release(&mut f, Some(17));
    assert_eq!((f.reads, f.acquisitions, f.releases), (2, 1, 1));
    assert_eq!(f.strong.load(SeqCst), 0);
    assert_eq!(f.destroyed, 0);
    f.source = Some(23);
    for count in [-1, -2, -3] {
        f.strong.store(count, SeqCst);
        assert_eq!(refs::get_ref(&mut f, Some(17)), None);
        assert_eq!(f.strong.load(SeqCst), count);
        assert_eq!(f.destroyed, 0);
    }
}

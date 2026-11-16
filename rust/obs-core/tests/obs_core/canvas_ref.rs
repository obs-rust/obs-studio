//! Safe counterpart of test_scene_canvas_characterization.c::canvas_refs.
use obs_core::canvas_ref::{acquire, addref, release, weak};
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

#[test]
fn canvas_refs_characterization() {
    assert_eq!(acquire::<usize>(None), None);
    assert_eq!(weak::<usize>(None), None);
    addref(None);
    assert!(!release(None));
    let refs = CLongAtomic::new(0);
    let weak_refs = CLongAtomic::new(0);
    let canvas = 17usize;
    assert_eq!(acquire(Some((&refs, canvas))), Some(canvas));
    assert_eq!(refs.fetch_sub(1, SeqCst), 1);
    assert_eq!(weak(Some((&weak_refs, 31usize))), Some(31));
    addref(Some(&weak_refs));
    assert!(!release(Some(&weak_refs)));
    assert_eq!(acquire(Some((&refs, canvas))), Some(canvas));
    assert_eq!(refs.fetch_sub(1, SeqCst), 1);
    assert_eq!(refs.fetch_sub(1, SeqCst), 0);
    assert!(!release(Some(&weak_refs))); // destroy drops the implicit weak owner
    assert_eq!(acquire(Some((&refs, canvas))), None);
    assert!(release(Some(&weak_refs)));
    assert_eq!(weak_refs.load(SeqCst), -1);
}

#[test]
fn expired_controls_never_resurrect() {
    for value in [-2, -1] {
        let refs = CLongAtomic::new(value);
        assert_eq!(acquire(Some((&refs, 19))), None);
        assert_eq!(refs.load(SeqCst), value);
    }
}

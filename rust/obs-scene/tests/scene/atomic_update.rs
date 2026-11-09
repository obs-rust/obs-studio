//! Safe equivalents of test_scene_atomic_update.c's callback/lifetime cases.
use obs_scene::atomic_update::{Scene, update};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Default)]
struct State {
    refs: Cell<usize>,
    removed: Cell<bool>,
    events: RefCell<Vec<&'static str>>,
}
struct Handle(Rc<State>, Cell<bool>);
impl Scene for Handle {
    fn acquire(&self) -> Option<Self> {
        self.0.events.borrow_mut().push("acquire");
        if self.0.refs.get() == 0 {
            return None;
        }
        self.0.refs.set(self.0.refs.get() + 1);
        Some(Self(self.0.clone(), Cell::new(true)))
    }
    fn lock(&self) {
        self.0.events.borrow_mut().extend(["video lock", "audio lock"]);
    }
    fn unlock(&self) {
        self.0.events.borrow_mut().extend(["audio unlock", "video unlock"]);
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if self.1.replace(false) {
            self.0.refs.set(self.0.refs.get() - 1);
            self.0.events.borrow_mut().push("release");
        }
    }
}

#[test]
fn null_scene() {
    update::<Handle>(None, |_| panic!("NULL scene called back"));
}

fn run_scene_case(removed: bool, release_caller: bool) {
    let state = Rc::new(State::default());
    state.refs.set(1);
    let scene = Handle(state.clone(), Cell::new(true));
    state.removed.set(removed);
    assert_eq!(state.removed.get(), removed);
    let calls = Cell::new(0);
    update(Some(&scene), |received| {
        calls.set(calls.get() + 1);
        assert!(Rc::ptr_eq(&received.0, &scene.0));
        assert_eq!(state.refs.get(), 2);
        assert_eq!(&*state.events.borrow(), &["acquire", "video lock", "audio lock"]);
        state.events.borrow_mut().push("callback");
        if release_caller {
            scene.1.set(false);
            state.refs.set(state.refs.get() - 1);
            assert!(state.refs.get() > 0);
        }
    });
    assert_eq!(calls.get(), 1);
    assert_eq!(&*state.events.borrow(), &["acquire", "video lock", "audio lock", "callback", "audio unlock", "video unlock", "release"]);
    drop(scene);
    assert_eq!(state.refs.get(), 0);
    // Storage remains readable in the destroy signal, although refs are expired.
    let expired = Handle(state.clone(), Cell::new(false));
    update(Some(&expired), |_| calls.set(calls.get() + 1));
    assert_eq!(calls.get(), 1);
    assert_eq!(state.refs.get(), 0);
    assert_eq!(state.events.borrow().last(), Some(&"acquire"));
    drop(expired);
    // Release the model's storage too, mirroring bnum_allocs restoration.
    assert_eq!(Rc::strong_count(&state), 1);
}

#[test]
fn live_scene_and_expired_scene() { run_scene_case(false, false); }
#[test]
fn removed_scene_and_expired_scene() { run_scene_case(true, false); }
#[test]
fn callback_holds_last_reference() { run_scene_case(false, true); }

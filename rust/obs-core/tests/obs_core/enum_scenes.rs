//! Safe 1:1 model of test_enum_scenes.c. Canvas traversal is a dependency;
//! production ports only the wrapper, not the traversal or ownership rules.
use obs_core::enum_scenes::enum_scenes;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Fixture {
    refs: [i64; 7],
    canvas_refs: i64,
    active: [i64; 7],
    showing: [i64; 7],
    populated: bool,
}

#[derive(Default)]
struct Visit {
    seen: Vec<usize>,
    held: Vec<usize>,
    stop: usize,
    acquire: bool,
}

impl Fixture {
    fn new(populated: bool) -> Self {
        Self {
            refs: [1; 7],
            canvas_refs: 2,
            active: [0; 7],
            showing: [0; 7],
            populated,
        }
    }

    fn direct(&mut self, canvas: usize, callback: usize, param: &mut Visit) {
        assert_eq!(canvas, 73, "borrow the actual main canvas");
        assert_eq!(callback, 19, "preserve callback identity");
        if !self.populated {
            return;
        }
        // Three public scenes and a group, in creation order. Private scene
        // 4, other-canvas scene 5 and non-scene input 6 are not enumerated.
        let before = self.refs;
        for (i, initial) in before.iter().copied().take(4).enumerate() {
            self.refs[i] += 1;
            assert_eq!(self.refs[i], initial + 1, "temporary callback reference");
            param.seen.push(i);
            if param.acquire {
                self.refs[i] += 1;
                param.held.push(i);
            }
            let keep_going = param.stop == 0 || param.seen.len() < param.stop;
            self.refs[i] -= 1;
            if !keep_going {
                break;
            }
        }
    }

    fn call(&mut self, v: &mut Visit) {
        enum_scenes(73, 19, v, |c, cb, param| self.direct(c, cb, param));
    }
}

#[test]
fn test_enum_scenes_empty_canvas() {
    let mut f = Fixture::new(false);
    let before = f.clone();
    let mut v = Visit::default();
    f.call(&mut v);
    assert!(v.seen.is_empty());
    f.direct(73, 19, &mut v);
    assert!(v.seen.is_empty());
    assert_eq!(f, before);
}

#[test]
fn test_enum_scenes_filtering_order_and_ownership() {
    let mut f = Fixture::new(true);
    let before = f.clone();
    let mut direct = Visit::default();
    f.direct(73, 19, &mut direct);
    assert_eq!(direct.seen, [0, 1, 2, 3]);
    assert_eq!(f, before);
    for stop in 0..=4 {
        let mut v = Visit {
            stop,
            acquire: true,
            ..Visit::default()
        };
        f.call(&mut v);
        let count = if stop == 0 { 4 } else { stop };
        assert_eq!(v.seen, direct.seen[..count]);
        assert_eq!(v.held, direct.seen[..count]);
        for i in v.held {
            assert_eq!(f.refs[i], before.refs[i] + 1);
            f.refs[i] -= 1;
        }
        assert_eq!(
            f, before,
            "balanced refs, unchanged activation/showing and canvas ownership"
        );
    }
}

#[test]
fn forwards_each_current_canvas_callback_and_opaque_parameter_once() {
    for canvas in [0, 1, 73, usize::MAX] {
        for callback in [0, 19, usize::MAX] {
            for param in [0, 7, usize::MAX] {
                let mut calls = Vec::new();
                enum_scenes(canvas, callback, param, |c, cb, p| calls.push((c, cb, p)));
                assert_eq!(calls, [(canvas, callback, param)]);
            }
        }
    }
}

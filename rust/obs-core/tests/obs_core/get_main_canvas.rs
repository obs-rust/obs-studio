//! Safe counterparts of the unchanged C characterization in #4779.
use obs_core::get_main_canvas::get_main_canvas;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Canvas {
    refs: i32,
    weak_refs: i32,
    name: &'static str,
    removed: bool,
    video: Option<usize>,
}

struct Fixture {
    main: Option<usize>,
    canvases: [Canvas; 3],
    calls: usize,
}

impl Fixture {
    fn new() -> Self {
        Self {
            main: Some(0),
            canvases: ["Main", "public", "private"].map(|name| Canvas {
                refs: 0,
                weak_refs: 0,
                name,
                removed: false,
                video: None,
            }),
            calls: 0,
        }
    }

    fn acquire(&mut self, canvas: Option<usize>) -> Option<usize> {
        self.calls += 1;
        if let Some(index) = canvas {
            self.canvases[index].refs += 1;
        }
        canvas
    }

    fn get(&mut self) -> Option<usize> {
        get_main_canvas(self.main, |canvas| self.acquire(canvas))
    }

    fn release(&mut self, canvas: Option<usize>) {
        if let Some(index) = canvas {
            self.canvases[index].refs -= 1;
        }
    }
}

#[test]
fn identity_and_acquired_reference() {
    let mut f = Fixture::new();
    let expected = f.acquire(Some(0)); // The C test's acquired name lookup.
    let before = f.canvases[0].clone();
    let first = f.get();
    assert_eq!(first, expected);
    assert_eq!(f.canvases[0].refs, before.refs + 1);
    let second = f.get();
    assert_eq!(second, expected);
    assert_eq!(f.canvases[0].refs, before.refs + 2);
    assert_eq!(f.canvases[0].weak_refs, before.weak_refs);
    assert_eq!(f.canvases[0].name, "Main");
    assert!(!f.canvases[0].removed);
    assert_eq!(f.canvases[0].video, None);
    f.release(first);
    assert_eq!(f.canvases[0].refs, before.refs + 1);
    f.release(second);
    assert_eq!(f.canvases[0], before);
    f.release(expected);
    assert_eq!(f.canvases[0].refs, 0);
    assert_eq!(f.calls, 3);
}

#[test]
fn repeated_reads_preserve_core_ownership() {
    let mut f = Fixture::new();
    let canvas = f.get();
    f.canvases[0].weak_refs += 1;
    let before = f.canvases[0].clone();
    let held: [_; 8] = std::array::from_fn(|i| {
        let read = f.get();
        assert_eq!(read, canvas);
        assert_eq!(f.canvases[0].refs, before.refs + i as i32 + 1);
        assert_eq!(f.canvases[0].weak_refs, before.weak_refs);
        read
    });
    for (i, read) in held.into_iter().enumerate().rev() {
        f.release(read);
        assert_eq!(f.canvases[0].refs, before.refs + i as i32);
    }
    f.release(canvas);
    assert_eq!(f.canvases[0].refs, before.refs - 1);
    let from_weak = f.acquire(canvas);
    assert_eq!(f.canvases[0].refs, before.refs);
    let again = f.get();
    assert_eq!(again, from_weak);
    f.release(again);
    f.release(from_weak);
    f.canvases[0].weak_refs -= 1;
    assert_eq!(f.canvases[0].refs, 0);
    assert_eq!(f.canvases[0].weak_refs, 0);
}

#[test]
fn auxiliary_canvases_do_not_change_main() {
    let mut f = Fixture::new();
    let main = f.get();
    assert_ne!(main, Some(1));
    assert_ne!(main, Some(2));
    let before = f.canvases.clone();
    let read = f.get();
    assert_eq!(read, main);
    assert_eq!(f.canvases[0].refs, before[0].refs + 1);
    assert_eq!(f.canvases[1..], before[1..]);
    f.release(read);
    f.canvases[1].removed = true;
    f.canvases[2].removed = true;
    let read = f.get();
    assert_eq!(read, main);
    f.release(read);
    assert_eq!(f.canvases[0], before[0]);
    f.release(main);
}

#[test]
fn forwards_current_canvas_once_and_returns_downstream_identity() {
    for current in [None, Some(0), Some(1), Some(usize::MAX)] {
        for returned in [None, Some(0), Some(2), Some(usize::MAX)] {
            let mut calls = 0;
            let result = get_main_canvas(current, |received| {
                calls += 1;
                assert_eq!(received, current);
                returned
            });
            assert_eq!(result, returned);
            assert_eq!(calls, 1);
        }
    }
}

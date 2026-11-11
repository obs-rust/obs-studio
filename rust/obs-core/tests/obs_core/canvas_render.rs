//! Safe delegation contract mirrored from test_canvas_render.c. The unchanged
//! Tier 2 test exercises the real view/source renderer and graphics context.
use obs_core::canvas_render::render;
use std::cell::{Cell, RefCell};

struct Canvas {
    channels: RefCell<[Option<usize>; 64]>,
    flags: Cell<u32>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            channels: RefCell::new([None; 64]),
            flags: Cell::new(0),
        }
    }
}

#[test]
fn empty_view() {
    for graphics in [false, true] {
        let canvases = [Canvas::default(), Canvas::default()];
        let calls = Cell::new(0);
        let linear_srgb = Cell::new(true);
        let context = (graphics, usize::from(graphics), 0, 0, 0, None::<usize>);
        render(
            1,
            |index| &canvases[index],
            |view| {
                assert!(std::ptr::eq(view, &canvases[1]));
                calls.set(calls.get() + 1);
                assert!(view.channels.borrow().iter().all(Option::is_none));
            },
        );
        assert_eq!(calls.get(), 1);
        assert!(linear_srgb.get());
        assert_eq!(context, (graphics, usize::from(graphics), 0, 0, 0, None));
        assert!(canvases[0].channels.borrow().iter().all(Option::is_none));
    }
}

#[test]
fn delegation() {
    for graphics in [false, true] {
        let canvases = [Canvas::default(), Canvas::default()];
        let selected = &canvases[1];
        selected.channels.borrow_mut()[0] = Some(0);
        selected.channels.borrow_mut()[2] = Some(1);
        selected.channels.borrow_mut()[3] = Some(0);
        selected.flags.set(16);
        let refs = [Cell::new(10), Cell::new(10)];
        let linear_srgb = Cell::new(true);
        let trace = RefCell::new(Vec::new());
        let access = Cell::new(0);
        let locked = Cell::new(false);
        let render_view = |view: &Canvas| {
            assert!(std::ptr::eq(view, selected));
            assert!(!locked.replace(true));
            for channel in 0..64 {
                let source = view.channels.borrow()[channel];
                if let Some(id) = source {
                    refs[id].set(refs[id].get() + 1);
                    assert_eq!(refs[id].get(), 11);
                    assert!(locked.get());
                    trace.borrow_mut().push(id);
                    if id == 0 {
                        view.channels.borrow_mut()[63] = Some(1);
                    }
                    view.flags.set(view.flags.get() + 1);
                    if graphics {
                        linear_srgb.set(false);
                    }
                    refs[id].set(refs[id].get() - 1);
                }
            }
            assert!(locked.replace(false));
        };
        render(
            1,
            |index| {
                access.set(access.get() + 1);
                &canvases[index]
            },
            render_view,
        );
        assert_eq!(access.get(), 1);
        assert_eq!(*trace.borrow(), [0, 1, 0, 1]);
        assert_eq!(selected.flags.get(), 20);
        assert_eq!(refs.map(|r| r.get()), [10, 10]);
        assert_eq!(selected.channels.borrow()[63], Some(1));
        assert_eq!(linear_srgb.get(), !graphics);
        assert!(!locked.get());
        assert!(canvases[0].channels.borrow().iter().all(Option::is_none));
    }
}

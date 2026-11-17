//! Tier 1 mirrors test_canvas_signals.c; caller lifecycle operations are modeled
//! explicitly, while the unchanged C suite exercises their real API paths.
use obs_callback::Calldata;
use obs_core::canvas_signals::{self as core, CAPACITY, Host, Route};
use std::ffi::CStr;

#[derive(Debug, PartialEq, Eq)]
struct Event(Route, Vec<u8>, usize, usize);
struct Canvas {
    private: bool,
    id: usize,
    stack: [u8; CAPACITY],
    size: usize,
    source: usize,
    events: Vec<Event>,
}
impl Canvas {
    fn new(private: bool, id: usize) -> Self {
        Self {
            private,
            id,
            stack: [0; CAPACITY],
            size: 0,
            source: 0,
            events: vec![],
        }
    }
}
impl Host for Canvas {
    fn init(&mut self, source: Option<usize>) {
        self.size = core::payload(&mut self.stack, self.id, source);
        self.source = source.unwrap_or(0);
        let mut expected_stack = [0; CAPACITY];
        let mut expected = Calldata::fixed(&mut expected_stack);
        expected.set_ptr("canvas", self.id);
        if let Some(source) = source {
            expected.set_ptr("source", source);
        }
        assert!(expected.is_fixed());
        assert_eq!(expected.capacity(), 128);
        assert_eq!(self.size, expected.size());
        assert_eq!(
            &self.stack[..self.size],
            &expected.stack().unwrap()[..self.size]
        );
    }
    fn private(&self) -> bool {
        self.private
    }
    fn signal(&mut self, route: Route, name: &CStr) {
        assert!(self.size <= 128);
        self.events
            .push(Event(route, name.to_bytes().to_vec(), self.id, self.source));
    }
}
fn event(route: Route, name: &CStr, canvas: usize, source: usize) -> Event {
    Event(route, name.to_bytes().to_vec(), canvas, source)
}
#[test]
fn global_only_creation() {
    let mut public = Canvas::new(false, 17);
    core::emit(&mut public, Some(c"canvas_create"), None);
    assert_eq!(
        public.events,
        [event(Route::Global, c"canvas_create", 17, 0)]
    );
    let mut private = Canvas::new(true, 18);
    core::emit(&mut private, Some(c"canvas_create"), None);
    assert!(private.events.is_empty());
}
fn lifecycle(private: bool) {
    let mut canvas = Canvas::new(private, 19);
    let unrelated = Canvas::new(true, 20);
    core::emit(&mut canvas, Some(c"canvas_remove"), Some(c"remove"));
    let per_event = if private { 1 } else { 2 };
    assert_eq!(canvas.events.len(), per_event);
    let mut expected = vec![];
    if !private {
        expected.push(event(Route::Global, c"canvas_remove", 19, 0));
    }
    expected.push(event(Route::Local, c"remove", 19, 0));
    assert_eq!(canvas.events, expected);
    // obs_canvas_remove suppresses its second helper call, outside this unit.
    assert_eq!(canvas.events.len(), per_event);
    core::emit(&mut canvas, Some(c"canvas_destroy"), Some(c"destroy"));
    if !private {
        expected.push(event(Route::Global, c"canvas_destroy", 19, 0));
    }
    expected.push(event(Route::Local, c"destroy", 19, 0));
    assert_eq!(canvas.events.len(), 2 * per_event);
    assert_eq!(canvas.events, expected);
    assert!(unrelated.events.is_empty());
}
#[test]
fn public_lifecycle() {
    lifecycle(false);
}
#[test]
fn private_lifecycle() {
    lifecycle(true);
}
fn source_routes(private: bool) {
    let mut canvas = Canvas::new(private, 21);
    core::emit_source(&mut canvas, c"source_add", 22);
    assert_eq!(canvas.events, [event(Route::Local, c"source_add", 21, 22)]);
    core::emit_source(&mut canvas, c"source_remove", 22);
    assert_eq!(
        canvas.events,
        [
            event(Route::Local, c"source_add", 21, 22),
            event(Route::Local, c"source_remove", 21, 22)
        ]
    );
}
#[test]
fn public_source_payload() {
    source_routes(false);
}
#[test]
fn private_source_payload() {
    source_routes(true);
}
#[test]
fn optional_routes_and_null_source() {
    for private in [false, true] {
        for global in [None, Some(c"g")] {
            for local in [None, Some(c"l")] {
                let mut canvas = Canvas::new(private, 1);
                core::emit(&mut canvas, global, local);
                assert_eq!(
                    canvas.events.len(),
                    usize::from(global.is_some() && !private) + usize::from(local.is_some())
                );
            }
        }
    }
    let mut canvas = Canvas::new(false, 1);
    core::emit_source(&mut canvas, c"source_add", 0);
    assert_eq!(canvas.events, [event(Route::Local, c"source_add", 1, 0)]);
}

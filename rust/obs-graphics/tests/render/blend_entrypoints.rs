//! Tier 1 mirrors test_graphics_blend_entrypoints.c, with no FFI or unsafe code.
use obs_graphics::blend_entrypoints::{self as entry, Context};
use obs_graphics::state_stacks::{self, BlendState};

pub const DEFAULT: BlendState = BlendState {
    enabled: true, src_c: 4, dest_c: 5, src_a: 1, dest_a: 5, op: 0,
};
pub const SAVED: BlendState = BlendState {
    enabled: false, src_c: 1, dest_c: 0, src_a: 4, dest_a: 1, op: 1,
};
#[derive(Debug, PartialEq, Eq)]
pub enum Call { Enable(bool), Factors([u32; 4]), Operation(u32) }
#[derive(Default)]
pub struct Mock {
    pub current: BlendState,
    pub stack: Vec<BlendState>,
    pub calls: Vec<Call>,
}
impl Context for Mock {
    fn current(&self) -> BlendState { self.current }
    fn push(&mut self, state: BlendState) { state_stacks::state_push(&mut self.stack, state); }
    fn pop(&mut self) -> Option<BlendState> { state_stacks::state_pop(&mut self.stack) }
    fn enable(&mut self, value: bool) {
        self.current.enabled = value;
        self.calls.push(Call::Enable(value));
    }
    fn factors(&mut self, state: BlendState) {
        self.current.src_c = state.src_c;
        self.current.dest_c = state.dest_c;
        self.current.src_a = state.src_a;
        self.current.dest_a = state.dest_a;
        self.calls.push(Call::Factors([state.src_c, state.dest_c, state.src_a, state.dest_a]));
    }
    fn operation(&mut self, value: u32) {
        self.current.op = value;
        self.calls.push(Call::Operation(value));
    }
}
fn restoration(state: BlendState) -> Vec<Call> {
    vec![Call::Enable(state.enabled), Call::Factors([state.src_c, state.dest_c, state.src_a, state.dest_a]), Call::Operation(state.op)]
}
#[test]
fn test_graphics_blend_entrypoints_push_pop() {
    entry::push::<Mock>(None);
    entry::pop::<Mock>(None);
    entry::reset::<Mock>(None);
    let mut m = Mock { current: SAVED, ..Default::default() };
    entry::pop(Some(&mut m));
    assert!(m.calls.is_empty());
    entry::push(Some(&mut m));
    assert_eq!(m.stack, [SAVED]);
    assert!(m.calls.is_empty());
    m.current = DEFAULT;
    entry::push(Some(&mut m));
    assert_eq!(m.stack, [SAVED, DEFAULT]);
    entry::pop(Some(&mut m));
    assert_eq!(m.calls, restoration(DEFAULT));
    assert_eq!(m.current, DEFAULT);
    m.calls.clear();
    entry::pop(Some(&mut m));
    assert_eq!(m.calls, restoration(SAVED));
    assert_eq!(m.current, SAVED);
    assert!(m.stack.is_empty());
    m.calls.clear();
    entry::pop(Some(&mut m));
    assert!(m.calls.is_empty());
}
#[test]
fn test_graphics_blend_entrypoints_reset() {
    for mask in 0..8 {
        let mut m = Mock { current: DEFAULT, ..Default::default() };
        if mask & 1 != 0 { m.current.enabled = false; }
        if mask & 2 != 0 { m.current.src_c = 0; }
        if mask & 4 != 0 { m.current.op = 1; }
        let mut expected = Vec::new();
        if mask & 1 != 0 { expected.push(Call::Enable(true)); }
        if mask & 2 != 0 { expected.push(Call::Factors([4, 5, 1, 5])); }
        if mask & 6 != 0 { expected.push(Call::Operation(0)); }
        entry::reset(Some(&mut m));
        assert_eq!(m.calls, expected);
        assert_eq!(m.current, DEFAULT);
        m.calls.clear();
        entry::reset(Some(&mut m));
        assert!(m.calls.is_empty());
    }
}

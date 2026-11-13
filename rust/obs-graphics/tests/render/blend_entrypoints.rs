//! Tier 1 mirrors test_graphics_blend_entrypoints.c and the blend part of
//! test_raster_state_entrypoints.c, with no FFI or unsafe code.
use obs_graphics::blend_entrypoints::{self as entry, Context};
use obs_graphics::state_stacks::{self, BlendState};

pub const DEFAULT: BlendState = BlendState {
    enabled: true,
    src_c: 4,
    dest_c: 5,
    src_a: 1,
    dest_a: 5,
    op: 0,
};
pub const SAVED: BlendState = BlendState {
    enabled: false,
    src_c: 1,
    dest_c: 0,
    src_a: 4,
    dest_a: 1,
    op: 1,
};
#[derive(Debug, PartialEq, Eq)]
pub enum Call {
    Enable(bool),
    Function([u32; 2]),
    Factors([u32; 4]),
    Operation(u32),
}
/// A model context and recording device: the device calls only record, with
/// the blend state as the device saw it; the setters own the stores.
#[derive(Default)]
pub struct Mock {
    pub current: BlendState,
    pub stack: Vec<BlendState>,
    pub calls: Vec<Call>,
    pub seen: Vec<BlendState>,
}
impl Mock {
    fn record(&mut self, call: Call) {
        self.calls.push(call);
        self.seen.push(self.current);
    }
}
impl Context for Mock {
    fn current(&self) -> BlendState {
        self.current
    }
    fn current_mut(&mut self) -> &mut BlendState {
        &mut self.current
    }
    fn push(&mut self, state: BlendState) {
        state_stacks::state_push(&mut self.stack, state);
    }
    fn pop(&mut self) -> Option<BlendState> {
        state_stacks::state_pop(&mut self.stack)
    }
    fn enable(&mut self, value: bool) {
        self.record(Call::Enable(value));
    }
    fn function(&mut self, src: u32, dest: u32) {
        self.record(Call::Function([src, dest]));
    }
    fn factors(&mut self, src_c: u32, dest_c: u32, src_a: u32, dest_a: u32) {
        self.record(Call::Factors([src_c, dest_c, src_a, dest_a]));
    }
    fn operation(&mut self, value: u32) {
        self.record(Call::Operation(value));
    }
}
fn restoration(state: BlendState) -> Vec<Call> {
    vec![
        Call::Enable(state.enabled),
        Call::Factors([state.src_c, state.dest_c, state.src_a, state.dest_a]),
        Call::Operation(state.op),
    ]
}
#[test]
fn test_graphics_blend_entrypoints_push_pop() {
    entry::push::<Mock>(None);
    entry::pop::<Mock>(None);
    entry::reset::<Mock>(None);
    let mut m = Mock {
        current: SAVED,
        ..Default::default()
    };
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
        let mut m = Mock {
            current: DEFAULT,
            ..Default::default()
        };
        if mask & 1 != 0 {
            m.current.enabled = false;
        }
        if mask & 2 != 0 {
            m.current.src_c = 0;
        }
        if mask & 4 != 0 {
            m.current.op = 1;
        }
        let mut expected = Vec::new();
        if mask & 1 != 0 {
            expected.push(Call::Enable(true));
        }
        if mask & 2 != 0 {
            expected.push(Call::Factors([4, 5, 1, 5]));
        }
        if mask & 6 != 0 {
            expected.push(Call::Operation(0));
        }
        entry::reset(Some(&mut m));
        assert_eq!(m.calls, expected);
        assert_eq!(m.current, DEFAULT);
        m.calls.clear();
        entry::reset(Some(&mut m));
        assert!(m.calls.is_empty());
    }
}

// test_raster_state_entrypoints.c's initial state: DSTCOLOR, INVDSTCOLOR,
// DSTALPHA, INVDSTALPHA, OP_MAX, blending off.
const INITIAL: BlendState = BlendState {
    enabled: false,
    src_c: 6,
    dest_c: 7,
    src_a: 8,
    dest_a: 9,
    op: 4,
};

#[test]
fn test_raster_state_entrypoints_blend_missing_context() {
    // Without a context there is no state to store and no device to call:
    // each setter returns. (The adapter that found no context writes the
    // debug line; Tier 3 compares it with the C.)
    entry::enable_blending::<Mock>(None, true);
    entry::blend_function::<Mock>(None, 1, 0);
    entry::blend_function_separate::<Mock>(None, 1, 0, 4, 5);
    entry::blend_op::<Mock>(None, 1);
}

/// The one call made, the state the device saw, and the state left behind.
fn expect_only(m: &mut Mock, call: Call, expected: BlendState) {
    assert_eq!(m.calls, [call]);
    // The state is stored before the device is told.
    assert_eq!(m.seen, [expected]);
    assert_eq!(m.current, expected);
    m.calls.clear();
    m.seen.clear();
}

#[test]
fn test_raster_state_entrypoints_blend_forwarding() {
    let mut m = Mock {
        current: INITIAL,
        ..Default::default()
    };
    let mut expected = INITIAL;
    for i in 0..3 {
        let enable = i != 1;
        entry::enable_blending(Some(&mut m), enable);
        expected.enabled = enable;
        expect_only(&mut m, Call::Enable(enable), expected);
    }
    // ZERO/ONE, SRCALPHA/INVSRCALPHA, SRCALPHASAT/DSTCOLOR, undeclared.
    for [src, dest] in [[0, 1], [4, 5], [10, 6], [42, 0x7fff]] {
        entry::blend_function(Some(&mut m), src, dest);
        // One pair sets both the colour and the alpha factors.
        expected.src_c = src;
        expected.src_a = src;
        expected.dest_c = dest;
        expected.dest_a = dest;
        expect_only(&mut m, Call::Function([src, dest]), expected);
    }
    for quad in [
        [1, 0, 4, 5],
        [2, 3, 8, 9],
        [7, 6, 1, 10],
        [99, 300, 1000, 0],
    ] {
        let [src_c, dest_c, src_a, dest_a] = quad;
        entry::blend_function_separate(Some(&mut m), src_c, dest_c, src_a, dest_a);
        expected.src_c = src_c;
        expected.dest_c = dest_c;
        expected.src_a = src_a;
        expected.dest_a = dest_a;
        expect_only(&mut m, Call::Factors(quad), expected);
    }
    // ADD, SUBTRACT, REVERSE_SUBTRACT, MIN, MAX, undeclared.
    for op in [0, 1, 2, 3, 4, 17] {
        entry::blend_op(Some(&mut m), op);
        expected.op = op;
        // The device gets the operation as stored.
        expect_only(&mut m, Call::Operation(op), expected);
    }
}

#[test]
fn test_raster_state_entrypoints_push_pop_sees_setters() {
    let mut m = Mock {
        current: INITIAL,
        ..Default::default()
    };
    // What the setters store is what push saves ...
    entry::enable_blending(Some(&mut m), true);
    entry::blend_function_separate(Some(&mut m), 1, 0, 4, 5);
    entry::blend_op(Some(&mut m), 2);
    let saved = BlendState {
        enabled: true,
        src_c: 1,
        dest_c: 0,
        src_a: 4,
        dest_a: 5,
        op: 2,
    };
    assert_eq!(m.current, saved);
    entry::push(Some(&mut m));
    assert_eq!(m.stack, [saved]);

    // ... and pop restores it after the setters change it.
    entry::enable_blending(Some(&mut m), false);
    entry::blend_function(Some(&mut m), 6, 7);
    entry::blend_op(Some(&mut m), 3);
    m.calls.clear();
    m.seen.clear();
    entry::pop(Some(&mut m));
    assert!(m.stack.is_empty());
    assert_eq!(m.current, saved);
    assert_eq!(m.calls, restoration(saved));

    // gs_blend_function's single pair is pushed as colour and alpha alike.
    entry::blend_function(Some(&mut m), 2, 3);
    entry::push(Some(&mut m));
    entry::blend_function_separate(Some(&mut m), 0, 0, 0, 0);
    m.calls.clear();
    entry::pop(Some(&mut m));
    assert_eq!(m.calls[1], Call::Factors([2, 3, 2, 3]));
}

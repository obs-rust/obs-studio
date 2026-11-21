//! Tier 1: every assertion of the unchanged test_hotkey_dispatch.c against
//! the safe dispatch. The fake core owns one hotkey, one binding and the
//! routing fields; its callback and router check their arguments as the C
//! recorders do. A NULL hotkey is `None`, and touching it panics.
use obs_hotkey::dispatch::{Dispatch, dispatch};

const INVALID_ID: usize = usize::MAX;
const CALLBACK_DATA: usize = 0xca11;
const ROUTER_DATA: usize = 0x2007;

#[derive(Clone, Copy)]
struct Key {
    id: usize,
    func: bool,
    data: usize,
    pressed: i32,
}

#[derive(Clone, Copy)]
struct Binding {
    pressed: bool,
    modifiers_match: bool,
    hotkey: Option<usize>,
    hotkey_id: usize,
}

struct Core {
    reroute: bool,
    router: bool,
    router_data: usize,
    key: Key,
    binding: Binding,
    bindings: usize,
    callback_calls: usize,
    router_calls: usize,
    events: [bool; 8],
}

/// The hotkey handle: `Some(0)` is the test's hotkey, `None` is NULL.
type Handle = Option<usize>;

impl Core {
    /// `setup()`: zero state, a hotkey with id 42 and its callback, and a
    /// binding of it.
    fn setup() -> Self {
        Core {
            reroute: false,
            router: false,
            router_data: 0,
            key: Key {
                id: 42,
                func: true,
                data: CALLBACK_DATA,
                pressed: 0,
            },
            binding: Binding {
                pressed: false,
                modifiers_match: false,
                hotkey: Some(0),
                hotkey_id: 42,
            },
            bindings: 0,
            callback_calls: 0,
            router_calls: 0,
            events: [false; 8],
        }
    }

    fn key(&self, hotkey: Handle) -> Key {
        assert_eq!(hotkey, Some(0), "a NULL hotkey was dereferenced");
        self.key
    }
}

impl Dispatch for Core {
    type Hotkey = Handle;
    fn rerouting(&mut self) -> bool {
        self.reroute
    }
    fn has_router(&mut self) -> bool {
        self.router
    }
    /// `record_callback()`.
    fn call(&mut self, hotkey: Handle, pressed: bool) {
        let key = self.key(hotkey);
        assert!(key.func, "a NULL callback was called");
        assert_eq!(key.data, CALLBACK_DATA);
        assert_eq!(key.id, self.key.id);
        assert_eq!(hotkey, Some(0));
        assert!(self.callback_calls < self.events.len());
        self.events[self.callback_calls] = pressed;
        self.callback_calls += 1;
    }
    /// `record_router()`.
    fn route(&mut self, hotkey: Handle, pressed: bool) {
        let key = self.key(hotkey);
        assert!(self.router, "a NULL router was called");
        assert_eq!(self.router_data, ROUTER_DATA);
        assert_eq!(key.id, self.key.id);
        assert!(self.router_calls < self.events.len());
        self.events[self.router_calls] = pressed;
        self.router_calls += 1;
    }
}

#[test]
fn active_binding_delivers_without_mutation() {
    let mut core = Core::setup();
    core.binding.pressed = true;
    core.binding.modifiers_match = true;
    core.key.pressed = 2;
    core.bindings = 1;
    // Even an installed router is ignored unless rerouting is enabled.
    core.router = true;
    core.router_data = ROUTER_DATA;
    let hotkey = core.binding.hotkey;
    dispatch(&mut core, hotkey, true);
    assert_eq!(core.callback_calls, 1);
    assert_eq!(core.router_calls, 0);
    assert!(core.events[0]);
    assert!(core.binding.pressed);
    assert!(core.binding.modifiers_match);
    assert_eq!(core.key.pressed, 2);
    assert_eq!(core.bindings, 1);
    assert_eq!(core.binding.hotkey, Some(0));
}

#[test]
fn absent_binding_still_delivers() {
    let mut core = Core::setup();
    // No registry or matching binding: dispatch performs no lookup.
    core.key.id = INVALID_ID;
    assert_eq!(core.bindings, 0);
    dispatch(&mut core, Some(0), true);
    assert_eq!(core.callback_calls, 1);
    assert!(core.events[0]);
    assert_eq!(core.key.pressed, 0);
    assert!(!core.binding.pressed);
    assert_eq!(core.bindings, 0);
    // An unrelated binding must not suppress delivery either.
    core.binding.hotkey_id = 7;
    core.binding.hotkey = None;
    core.bindings = 1;
    dispatch(&mut core, Some(0), false);
    assert_eq!(core.callback_calls, 2);
    assert!(!core.events[1]);
    assert_eq!(core.binding.hotkey_id, 7);
    assert_eq!(core.binding.hotkey, None);
}

#[test]
fn press_release_and_duplicates_are_forwarded() {
    let mut core = Core::setup();
    dispatch(&mut core, Some(0), true);
    dispatch(&mut core, Some(0), true);
    dispatch(&mut core, Some(0), false);
    dispatch(&mut core, Some(0), false);
    assert_eq!(core.callback_calls, 4);
    assert!(core.events[0]);
    assert!(core.events[1]);
    assert!(!core.events[2]);
    assert!(!core.events[3]);
    assert_eq!(core.key.pressed, 0);
    assert!(!core.binding.pressed);
}

#[test]
fn routing_uses_current_router_and_id() {
    let mut core = Core::setup();
    core.reroute = true;
    core.router = true;
    core.router_data = ROUTER_DATA;
    // Routed dispatch does not inspect the hotkey's callback.
    core.key.func = false;
    core.key.data = 0;
    dispatch(&mut core, Some(0), true);
    core.key.id = INVALID_ID;
    dispatch(&mut core, Some(0), false);
    assert_eq!(core.router_calls, 2);
    assert_eq!(core.callback_calls, 0);
    assert!(core.events[0]);
    assert!(!core.events[1]);
    core.router = false;
    dispatch(&mut core, Some(0), true);
    assert_eq!(core.router_calls, 2);
    core.key.func = true;
    core.key.data = CALLBACK_DATA;
    core.reroute = false;
    dispatch(&mut core, Some(0), false);
    assert_eq!(core.callback_calls, 1);
    assert!(!core.events[0]);
}

#[test]
fn zero_state_and_null_no_router_branch() {
    let mut core = Core::setup();
    // No startup or initialized mutex is needed by this function.
    dispatch(&mut core, Some(0), false);
    assert_eq!(core.callback_calls, 1);
    assert!(!core.events[0]);
    core.reroute = true;
    // This branch returns without touching even a NULL hotkey.
    dispatch(&mut core, None, true);
    dispatch(&mut core, None, false);
    assert_eq!(core.callback_calls, 1);
    assert_eq!(core.router_calls, 0);
    assert!(!core.router);
}

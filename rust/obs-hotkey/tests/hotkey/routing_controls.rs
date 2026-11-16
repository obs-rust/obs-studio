//! Safe configuration checks mirroring test_hotkey_press_state's rerouted
//! and rerouted_without_router setup, plus replacement and failed-lock edges.
use obs_hotkey::routing_controls::{self, RoutingControls};

struct Registry {
    present: bool,
    callback: Option<u8>,
    data: usize,
    enabled: bool,
    events: Vec<&'static str>,
}
impl RoutingControls for Registry {
    type Callback = Option<u8>;
    type Data = usize;
    fn lock(&mut self) -> bool {
        self.events.push("lock");
        self.present
    }
    fn set_callback(&mut self, callback: Self::Callback, data: Self::Data) {
        self.events.push("callback");
        self.callback = callback;
        self.data = data;
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.events.push("enabled");
        self.enabled = enabled;
    }
    fn unlock(&mut self) {
        self.events.push("unlock");
    }
}

#[test]
fn test_hotkey_press_state_rerouted_configuration() {
    let mut r = Registry {
        present: true,
        callback: None,
        data: 9,
        enabled: false,
        events: vec![],
    };
    for callback in [Some(1), Some(2), None] {
        for data in [0, 17, usize::MAX] {
            for enabled in [true, false, false, true] {
                routing_controls::set_callback(&mut r, callback, data);
                assert_eq!((r.callback, r.data), (callback, data));
                routing_controls::set_enabled(&mut r, enabled);
                assert_eq!((r.callback, r.data, r.enabled), (callback, data, enabled));
                assert_eq!(
                    r.events,
                    ["lock", "callback", "unlock", "lock", "enabled", "unlock"]
                );
                r.events.clear();
            }
        }
    }
}

#[test]
fn absent_core_preserves_all_configuration() {
    let mut r = Registry {
        present: false,
        callback: Some(3),
        data: 7,
        enabled: true,
        events: vec![],
    };
    routing_controls::set_callback(&mut r, None, 0);
    routing_controls::set_enabled(&mut r, false);
    assert_eq!((r.callback, r.data, r.enabled), (Some(3), 7, true));
    assert_eq!(r.events, ["lock", "lock"]);
}

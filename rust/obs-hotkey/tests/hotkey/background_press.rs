use obs_hotkey::background_press::{BackgroundPress, enable};

#[derive(Default)]
struct Client {
    present: bool,
    locked: bool,
    disabled: bool,
    setter_active: bool,
    expected_enabled: bool,
    locks: usize,
    unlocks: usize,
    signals: Vec<bool>,
}

impl BackgroundPress for Client {
    fn lock(&mut self) -> bool {
        assert!(!self.locked);
        self.locks += 1;
        self.locked = self.present;
        self.locked
    }
    fn disabled(&self) -> bool {
        assert!(self.locked);
        self.disabled
    }
    fn set_disabled(&mut self, disabled: bool) {
        assert!(self.locked);
        self.disabled = disabled;
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.unlocks += 1;
        self.locked = false;
    }
    fn signal(&mut self, enabled: bool) {
        assert!(self.setter_active);
        assert_eq!(enabled, self.expected_enabled);
        assert_eq!(self.disabled, !enabled);
        assert!(!self.locked);
        // Mirror the listener's getter re-entry: a second lock is available
        // synchronously, before the setter returns.
        assert!(self.lock());
        assert_eq!(self.disabled(), !enabled);
        self.unlock();
        self.signals.push(enabled);
    }
}

impl Client {
    fn request(&mut self, enabled: bool, calls: usize) {
        self.expected_enabled = enabled;
        self.setter_active = true;
        enable(self, enabled);
        self.setter_active = false;
        assert_eq!(self.signals.len(), calls);
        assert_eq!(self.disabled, !enabled);
        assert!(!self.locked);
    }
}

#[test]
fn mirrors_test_hotkey_background_press() {
    let mut client = Client::default();
    enable(&mut client, false);
    enable(&mut client, true);
    assert!(client.signals.is_empty());
    assert_eq!(client.unlocks, 0);
    client.present = true;
    for (enabled, calls) in [
        (true, 0),
        (false, 1),
        (false, 1),
        (true, 2),
        (true, 2),
        (false, 3),
        (true, 4),
    ] {
        client.request(enabled, calls);
    }
    assert_eq!(client.signals, [false, true, false, true]);
    assert_eq!(client.unlocks, 11);
    client.present = false;
    enable(&mut client, false);
    assert!(!client.disabled);
    assert_eq!(client.signals.len(), 4);
    assert!(!client.locked);
}

#[test]
fn initially_disabled_and_missing_core_preserve_state() {
    let mut client = Client {
        disabled: true,
        ..Client::default()
    };
    enable(&mut client, true);
    assert!(client.disabled);
    assert!(client.signals.is_empty());
    client.present = true;
    client.request(false, 0);
    client.request(true, 1);
    client.request(true, 1);
    assert_eq!(client.signals, [true]);
    assert_eq!(client.unlocks, 4);
}

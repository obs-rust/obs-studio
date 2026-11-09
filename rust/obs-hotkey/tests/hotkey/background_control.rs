use obs_hotkey::background_control::{BackgroundControl, Request, State, request};

#[derive(Default)]
struct Client {
    state: State,
    present: bool,
    callback: bool,
    locked: bool,
    locks: usize,
    unlocks: usize,
    wakes: usize,
}
impl BackgroundControl for Client {
    fn lock(&mut self) -> bool {
        assert!(!self.locked);
        self.locks += 1;
        self.locked = self.present;
        self.locked
    }
    fn read(&self) -> State {
        assert!(self.locked);
        self.state
    }
    fn write(&mut self, state: State) {
        assert!(self.locked);
        self.state = state;
    }
    fn wake(&mut self) {
        assert!(self.locked);
        self.wakes += usize::from(self.callback);
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.unlocks += 1;
        self.locked = false;
    }
}

#[test]
fn mirrors_test_hotkey_background_control() {
    let mut c = Client::default();
    for r in [Request::Retry, Request::Configure, Request::Probe(true)] {
        request(&mut c, r);
    }
    assert_eq!((c.locks, c.unlocks, c.wakes), (3, 0, 0));
    c.present = true;
    c.callback = true;
    request(&mut c, Request::Retry);
    request(&mut c, Request::Retry);
    assert!(c.state.retry);
    assert_eq!(c.wakes, 2);
    c.state.flags = u32::MAX;
    request(&mut c, Request::Configure);
    request(&mut c, Request::Configure);
    assert!(c.state.configure);
    assert_eq!(c.state.flags, !(1 << 4));
    assert_eq!(c.wakes, 4);
    request(&mut c, Request::Probe(false));
    assert!(c.state.configure);
    assert_eq!(c.wakes, 4);
    request(&mut c, Request::Probe(true));
    assert!(c.state.probe);
    assert!(c.state.configure);
    request(&mut c, Request::Probe(true));
    assert_eq!(c.wakes, 5);
    request(&mut c, Request::Probe(false));
    assert!(!c.state.probe);
    assert!(!c.state.configure);
    assert_eq!(c.wakes, 6);
    c.callback = false;
    for r in [Request::Retry, Request::Configure, Request::Probe(true)] {
        request(&mut c, r);
    }
    assert_eq!(c.wakes, 6);
    assert!(!c.locked);
    assert_eq!((c.locks, c.unlocks), (14, 11));
}

#[test]
fn preserves_unrelated_flags_and_retry_when_probe_ends() {
    let mut c = Client {
        present: true,
        state: State {
            retry: true,
            configure: true,
            probe: true,
            flags: u32::MAX,
        },
        ..Client::default()
    };
    request(&mut c, Request::Probe(false));
    assert_eq!(
        c.state,
        State {
            retry: true,
            configure: false,
            probe: false,
            flags: u32::MAX
        }
    );
}

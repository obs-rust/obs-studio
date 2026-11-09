use obs_source::push_hotkeys::{Host, Kind, event};

#[derive(Default)]
pub(super) struct World {
    pub now: u64,
    pub locked: bool,
    pub mute: bool,
    pub talk: bool,
    pub actions: Vec<(u64, Kind, bool)>,
    pub trace: Vec<u64>,
}
impl Host for World {
    fn clock(&mut self) -> u64 {
        assert!(!self.locked);
        self.trace.push(1);
        self.now
    }
    fn lock(&mut self) {
        assert!(!self.locked);
        self.trace.push(2);
        self.locked = true;
    }
    fn append(&mut self, timestamp: u64, kind: Kind, pressed: bool) {
        assert!(self.locked);
        self.trace.extend([3, u64::from(self.mute), u64::from(self.talk)]);
        self.actions.push((timestamp, kind, pressed));
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.trace.push(4);
        self.locked = false;
    }
    fn store(&mut self, kind: Kind, pressed: bool) {
        assert!(!self.locked);
        match kind {
            Kind::Mute => self.mute = pressed,
            Kind::Talk => self.talk = pressed,
        }
    }
}

#[test]
fn test_source_push_hotkeys_routed_actions() {
    let mut world = World::default();
    assert!(!world.mute);
    assert!(!world.talk);
    assert!(world.actions.is_empty());
    let talk = [false, true, false, false, true, false, true, true];
    let pressed = [false, true, true, true, true, false, false, false];
    let mut mute_flag = false;
    let mut talk_flag = false;
    let mut timestamps = [0; 8];
    for i in 0..8 {
        let before = 100 + i as u64;
        world.now = before;
        let kind = if talk[i] { Kind::Talk } else { Kind::Mute };
        event(&mut world, kind, pressed[i]);
        let after = world.now;
        if talk[i] { talk_flag = pressed[i]; } else { mute_flag = pressed[i]; }
        assert_eq!(world.mute, mute_flag);
        assert_eq!(world.talk, talk_flag);
        assert_eq!(world.actions.len(), i + 1);
        for j in 0..=i {
            let action = world.actions[j];
            assert_eq!(action.1, if talk[j] { Kind::Talk } else { Kind::Mute });
            assert_eq!(action.2, pressed[j]);
            if j == i {
                assert!(action.0 >= before);
                assert!(action.0 <= after);
                timestamps[j] = action.0;
            } else {
                assert_eq!(action.0, timestamps[j]);
            }
        }
    }
    // No enable flags are exposed to this core: disabled gates cannot suppress events.
}

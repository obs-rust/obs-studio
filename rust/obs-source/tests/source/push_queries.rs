use obs_source::push_queries::{self as query, PushFields};

struct Source {
    locked: bool,
    locks: usize,
    mute: bool,
    talk: bool,
    mute_delay: u64,
    talk_delay: u64,
    mute_pressed: bool,
    talk_pressed: bool,
}
impl PushFields for Source {
    fn lock(&mut self) {
        assert!(!self.locked);
        self.locked = true;
        self.locks += 1;
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.locked = false;
    }
    fn mute_enabled(&self) -> bool {
        assert!(self.locked);
        self.mute
    }
    fn talk_enabled(&self) -> bool {
        assert!(self.locked);
        self.talk
    }
    fn mute_delay(&self) -> u64 {
        assert!(self.locked);
        self.mute_delay
    }
    fn talk_delay(&self) -> u64 {
        assert!(self.locked);
        self.talk_delay
    }
}

#[test]
fn mirrors_c_null_defaults() {
    let mut logs = Vec::new();
    assert!(!query::mute_enabled::<Source>(None, |f, p| logs.push((f, p))));
    assert_eq!(
        query::mute_delay::<Source>(None, |f, p| logs.push((f, p))),
        0
    );
    assert!(!query::talk_enabled::<Source>(None, |f, p| logs.push((f, p))));
    assert_eq!(
        query::talk_delay::<Source>(None, |f, p| logs.push((f, p))),
        0
    );
    assert_eq!(
        logs,
        [
            (c"obs_source_push_to_mute_enabled", c"source"),
            (c"obs_source_get_push_to_mute_delay", c"source"),
            (c"obs_source_push_to_talk_enabled", c"source"),
            (c"obs_source_get_push_to_talk_delay", c"source"),
        ]
    );
}

#[test]
fn mirrors_c_independent_fields_and_lock_order() {
    let edges = [0, 1, 0x8000_0000_0000_0000, u64::MAX, 0xa5a5_5a5a_f00f_1234];
    for mute_delay in edges {
        for talk_delay in edges {
            for flags in 0..4 {
                let mut s = Source {
                    locked: false,
                    locks: 0,
                    mute: flags & 1 != 0,
                    talk: flags & 2 != 0,
                    mute_delay,
                    talk_delay,
                    mute_pressed: flags & 1 == 0,
                    talk_pressed: flags & 2 == 0,
                };
                for repeat in 0..2 {
                    assert_eq!(
                        query::mute_enabled(Some(&mut s), |_, _| panic!("log")),
                        flags & 1 != 0
                    );
                    assert!(!s.locked);
                    assert_eq!(
                        query::talk_enabled(Some(&mut s), |_, _| panic!("log")),
                        flags & 2 != 0
                    );
                    assert!(!s.locked);
                    assert_eq!(
                        query::mute_delay(Some(&mut s), |_, _| panic!("log")),
                        mute_delay
                    );
                    assert!(!s.locked);
                    assert_eq!(
                        query::talk_delay(Some(&mut s), |_, _| panic!("log")),
                        talk_delay
                    );
                    assert!(!s.locked);
                    assert_eq!(s.locks, (repeat + 1) * 4);
                    assert_eq!(s.mute_pressed, flags & 1 == 0);
                    assert_eq!(s.talk_pressed, flags & 2 == 0);
                    assert_eq!(
                        (s.mute, s.talk, s.mute_delay, s.talk_delay),
                        (flags & 1 != 0, flags & 2 != 0, mute_delay, talk_delay)
                    );
                }
            }
        }
    }
}

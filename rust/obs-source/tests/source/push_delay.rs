use core::ffi::CStr;
use obs_source::push_delay::{DelayFields, set_delay};
use obs_source::push_queries::PushFields;

#[derive(Default)]
struct Source {
    locked: bool,
    delays: [u64; 2],
    calls: [usize; 2],
}
impl PushFields for Source {
    fn lock(&mut self) {
        assert!(!self.locked);
        self.locked = true;
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.locked = false;
    }
    fn mute_enabled(&self) -> bool {
        false
    }
    fn talk_enabled(&self) -> bool {
        false
    }
    fn mute_delay(&self) -> u64 {
        self.delays[0]
    }
    fn talk_delay(&self) -> u64 {
        self.delays[1]
    }
}
impl DelayFields for Source {
    fn store_delay(&mut self, talk: bool, delay: u64) {
        assert!(self.locked);
        self.delays[usize::from(talk)] = delay;
    }
    fn signal_delay(&mut self, signal: &'static CStr, mut delay: i64) {
        assert!(self.locked);
        let talk = signal == c"push_to_talk_delay";
        assert_eq!(
            signal,
            if talk {
                c"push_to_talk_delay"
            } else {
                c"push_to_mute_delay"
            }
        );
        assert_eq!(delay as u64, self.delays[usize::from(talk)]);
        // Callback reentry on a distinct source completes before outer unlock.
        let mut other = Nested::default();
        set_delay(Some(&mut other), true, u64::MAX, |_, _| panic!());
        assert_eq!(other.0.delays[1], u64::MAX);
        assert_eq!(other.0.calls[1], 1);
        assert!(!other.0.locked);
        delay = 17; // calldata mutation must not be stored back
        assert_eq!(delay, 17);
        self.calls[usize::from(talk)] += 1;
    }
}
#[derive(Default)]
struct Nested(Source);
impl PushFields for Nested {
    fn lock(&mut self) {
        self.0.lock();
    }
    fn unlock(&mut self) {
        self.0.unlock();
    }
    fn mute_enabled(&self) -> bool {
        false
    }
    fn talk_enabled(&self) -> bool {
        false
    }
    fn mute_delay(&self) -> u64 {
        self.0.mute_delay()
    }
    fn talk_delay(&self) -> u64 {
        self.0.talk_delay()
    }
}
impl DelayFields for Nested {
    fn store_delay(&mut self, talk: bool, delay: u64) {
        self.0.store_delay(talk, delay);
    }
    fn signal_delay(&mut self, signal: &'static CStr, delay: i64) {
        assert!(self.0.locked);
        assert_eq!(signal, c"push_to_talk_delay");
        assert_eq!(delay, -1);
        assert_eq!(self.0.delays[1], u64::MAX);
        self.0.calls[1] += 1;
    }
}
#[test]
fn test_source_push_to_delay_delays() {
    let mut s = Source::default();
    let values = [
        0,
        0,
        1,
        u32::MAX as u64,
        u32::MAX as u64 + 1,
        i64::MAX as u64,
        i64::MAX as u64 + 1,
        u64::MAX,
        u64::MAX,
        0,
    ];
    let mut previous = 0;
    for (i, value) in values.into_iter().enumerate() {
        set_delay(Some(&mut s), false, value, |_, _| panic!());
        assert_eq!(s.calls, [i + 1, i]);
        assert_eq!(s.delays, [value, previous]);
        assert!(!s.locked);
        set_delay(Some(&mut s), true, value, |_, _| panic!());
        assert_eq!(s.calls, [i + 1, i + 1]);
        assert_eq!(s.delays, [value, value]);
        assert!(!s.locked);
        previous = value;
    }
}
#[test]
fn test_source_push_to_delay_null_source() {
    let mut calls = 0;
    for delay in [0, u64::MAX] {
        for talk in [false, true] {
            set_delay::<Source>(None, talk, delay, |name, parameter| {
                assert_eq!(parameter, c"source");
                assert_eq!(
                    name,
                    if talk {
                        c"obs_source_set_push_to_talk_delay"
                    } else {
                        c"obs_source_set_push_to_mute_delay"
                    }
                );
                calls += 1;
            });
        }
    }
    assert_eq!(calls, 4);
}

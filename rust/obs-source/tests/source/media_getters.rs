//! Tier 1 mirrors test_source_media_getters.c with safe fixtures only.
use core::{cell::Cell, ffi::CStr};
use obs_source::media_getters::{self as media, Source};
struct Fixture {
    valid: bool,
    flags: u32,
    mask: u8,
    duration: i64,
    time: i64,
    state: u32,
    calls: [Cell<u32>; 3],
    reenter: bool,
}
impl Fixture {
    fn new(valid: bool, flags: u32, mask: u8) -> Self {
        Self {
            valid,
            flags,
            mask,
            duration: -123,
            time: i64::MAX,
            state: 7,
            calls: Default::default(),
            reenter: false,
        }
    }
    fn call<T: Copy>(&self, index: usize, value: T) -> Option<T> {
        assert!(self.valid && self.flags & (1 << 13) != 0);
        if self.mask & (1 << index) == 0 {
            return None;
        }
        self.calls[index].set(self.calls[index].get() + 1);
        Some(value)
    }
}
impl Source for Fixture {
    fn has_data(&self) -> bool {
        self.valid
    }
    fn output_flags(&self) -> u32 {
        assert!(self.valid);
        self.flags
    }
    fn duration(&self) -> Option<i64> {
        let result = self.call(0, self.duration);
        if self.reenter && result.is_some() {
            assert_eq!(media::time(Some(self), no_log), self.time);
            assert_eq!(media::state(Some(self), no_log), self.state);
        }
        result
    }
    fn time(&self) -> Option<i64> {
        self.call(1, self.time)
    }
    fn state(&self) -> Option<u32> {
        self.call(2, self.state)
    }
}
fn no_log(_: &'static CStr) {
    panic!("unexpected diagnostic")
}
#[test]
fn mirrors_null_source() {
    let messages = core::cell::RefCell::new(Vec::new());
    let log = |name| messages.borrow_mut().push(name);
    assert_eq!(media::duration(None::<&Fixture>, log), 0);
    assert_eq!(media::time(None::<&Fixture>, log), 0);
    assert_eq!(media::state(None::<&Fixture>, log), 0);
    assert_eq!(
        *messages.borrow(),
        [
            c"obs_source_media_get_duration",
            c"obs_source_media_get_time",
            c"obs_source_media_get_state"
        ]
    );
}
#[test]
fn mirrors_admission() {
    for valid in [false, true] {
        for controllable in [false, true] {
            for mask in 0..8 {
                let f = Fixture::new(valid, if controllable { 1 << 13 } else { 0 }, mask);
                let admitted = valid && controllable;
                assert_eq!(
                    media::duration(Some(&f), no_log),
                    if admitted && mask & 1 != 0 {
                        f.duration
                    } else {
                        0
                    }
                );
                assert_eq!(
                    media::time(Some(&f), no_log),
                    if admitted && mask & 2 != 0 { f.time } else { 0 }
                );
                assert_eq!(
                    media::state(Some(&f), no_log),
                    if admitted && mask & 4 != 0 {
                        f.state
                    } else {
                        0
                    }
                );
                for i in 0..3 {
                    assert_eq!(
                        f.calls[i].get(),
                        u32::from(admitted && mask & (1 << i) != 0)
                    );
                }
            }
        }
    }
}
#[test]
fn mirrors_results_and_context() {
    let values = [0, 1, -1, i64::MIN, i64::MAX, 4294967296, -4294967297];
    for _ in 0..2 {
        let mut f = Fixture::new(true, 1 << 13, 7);
        for (i, value) in values.into_iter().enumerate() {
            f.duration = value;
            f.time = values[values.len() - 1 - i];
            assert_eq!(media::duration(Some(&f), no_log), f.duration);
            assert_eq!(media::time(Some(&f), no_log), f.time);
            assert_eq!(f.calls[0].get(), i as u32 + 1);
            assert_eq!(f.calls[1].get(), i as u32 + 1);
        }
        for state in 0..8 {
            f.state = state;
            assert_eq!(media::state(Some(&f), no_log), state);
            assert_eq!(f.calls[2].get(), state + 1);
        }
    }
}
#[test]
fn mirrors_synchronous_reentry() {
    let mut f = Fixture::new(true, 1 << 13, 7);
    f.reenter = true;
    assert_eq!(media::duration(Some(&f), no_log), f.duration);
    assert_eq!(f.calls.each_ref().map(Cell::get), [1, 1, 1]);
}

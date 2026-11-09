//! Tier 1 mirrors test_source_async_decoupled.c, using owned safe audio state.
use obs_source::async_decoupled::{self, Source};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Audio {
    decoupled: bool,
    timing_set: bool,
    timing_adjust: u64,
    last_size: usize,
    audio_ts: u64,
    next_ts: u64,
    buffers: [Vec<u8>; 8],
    unbuffered: bool,
    width: u32,
    height: u32,
    locked: bool,
    calls: Vec<&'static str>,
}
impl Audio {
    fn seeded(decoupled: bool) -> Self {
        Self {
            decoupled, timing_set: true, timing_adjust: 123456,
            last_size: 87, audio_ts: 987654, next_ts: 456789,
            buffers: std::array::from_fn(|i| if i == 1 { vec![] } else {
                [11, 22, 33, 44, 55][..1 + i % 5].to_vec()
            }),
            unbuffered: true, width: 37, height: 53, locked: false, calls: vec![],
        }
    }
}
impl Source for Audio {
    fn set_decoupled(&mut self, value: bool) {
        assert!(!self.locked);
        self.decoupled = value;
        self.calls.push("store");
    }
    fn lock_audio(&mut self) {
        assert!(self.decoupled && !self.locked);
        self.locked = true;
        self.calls.push("lock");
    }
    fn invalidate_timing(&mut self) {
        assert!(self.locked);
        self.timing_set = false;
        self.calls.push("invalidate");
    }
    fn reset_audio(&mut self, timestamp: u64) {
        assert!(self.locked && !self.timing_set);
        for buffer in &mut self.buffers { buffer.clear(); }
        self.last_size = 0;
        self.audio_ts = timestamp;
        self.next_ts = timestamp;
        self.calls.push("reset");
    }
    fn unlock_audio(&mut self) {
        assert!(self.locked);
        self.locked = false;
        self.calls.push("unlock");
    }
}

#[test]
fn mirrors_c_null_source() {
    let mut logs = 0;
    for value in [false, true] {
        async_decoupled::set::<Audio>(None, value, || logs += 1);
    }
    assert_eq!(logs, 2);
}

#[test]
fn mirrors_c_enabling_resets_even_when_already_enabled() {
    let mut audio = Audio::seeded(false);
    for previous in [false, true] {
        assert_eq!(audio.decoupled, previous);
        audio = Audio::seeded(previous);
        let mut expected = audio.clone();
        expected.decoupled = true;
        expected.timing_set = false;
        expected.last_size = 0;
        expected.audio_ts = 0;
        expected.next_ts = 0;
        for buffer in &mut expected.buffers { buffer.clear(); }
        expected.calls = vec!["store", "lock", "invalidate", "reset", "unlock"];
        async_decoupled::set(Some(&mut audio), true, || panic!("valid source"));
        assert_eq!(audio, expected);
    }
}

#[test]
fn mirrors_c_disabling_preserves_audio() {
    for previous in [false, true] {
        let mut audio = Audio::seeded(previous);
        let mut expected = audio.clone();
        expected.decoupled = false;
        expected.calls.push("store");
        async_decoupled::set(Some(&mut audio), false, || panic!("valid source"));
        assert_eq!(audio, expected);
    }
}

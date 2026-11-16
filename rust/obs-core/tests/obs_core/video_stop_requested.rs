//! Tier 1: every assertion of test_video_stop_requested.c, without FFI.
use obs_core::video_stop_requested::{Mixes, stop_requested};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Lock,
    Unlock,
    Count,
    Video(usize),
    Stopped(usize),
}

/// A mix list of `stopped.len()` mixes whose video is its index.
#[derive(Default)]
pub struct Model {
    pub stopped: Vec<bool>,
    pub locked: bool,
    pub calls: Vec<Call>,
}
impl Mixes for Model {
    type Video = usize;
    fn lock(&mut self) {
        assert!(!self.locked, "the stop check locks mixes_mutex once");
        self.locked = true;
        self.calls.push(Call::Lock);
    }
    fn unlock(&mut self) {
        assert!(self.locked, "unlock without lock");
        self.locked = false;
        self.calls.push(Call::Unlock);
    }
    fn count(&mut self) -> usize {
        assert!(self.locked, "mixes.num is read under mixes_mutex");
        self.calls.push(Call::Count);
        self.stopped.len()
    }
    fn video(&mut self, index: usize) -> usize {
        assert!(self.locked, "the mix array is read under mixes_mutex");
        self.calls.push(Call::Video(index));
        index
    }
    fn stopped(&mut self, video: usize) -> bool {
        // Every query runs with the mixes mutex held.
        assert!(self.locked, "video_output_stopped runs under mixes_mutex");
        self.calls.push(Call::Stopped(video));
        self.stopped[video]
    }
}

/// The calls the C makes for `count` mixes: one lock, `num` once, then each
/// mix's video and its query in order, and one unlock.
pub fn expected_calls(count: usize) -> Vec<Call> {
    let mut calls = vec![Call::Lock, Call::Count];
    for index in 0..count {
        calls.extend([Call::Video(index), Call::Stopped(index)]);
    }
    calls.push(Call::Unlock);
    calls
}

#[test]
fn stop_all_combinations() {
    // Include the empty list and every boolean vector up to four mixes.
    for count in 0..=4usize {
        for mask in 0..(1u32 << count) {
            let mut model = Model {
                stopped: (0..count).map(|i| mask & (1 << i) != 0).collect(),
                ..Model::default()
            };
            assert_eq!(stop_requested(&mut model), mask == (1 << count) - 1);
            // Characterized, not endorsed: do not short-circuit on false.
            let visits = model
                .calls
                .iter()
                .filter(|c| matches!(c, Call::Stopped(_)))
                .count();
            assert_eq!(visits, count);
            assert_eq!(model.calls, expected_calls(count));
            assert!(!model.locked, "mixes_mutex is released on return");
        }
    }
}

#[test]
fn a_running_first_mix_still_asks_the_rest() {
    let mut model = Model {
        stopped: vec![false, true, true, true, true, true, true, true],
        ..Model::default()
    };
    assert!(!stop_requested(&mut model));
    assert_eq!(model.calls, expected_calls(8));
}

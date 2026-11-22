//! Safe mirrors of test_video_active.c over a recording host: the mix list,
//! its mutex and each mix's raw_active and gpu_encoder_active counts. Plus
//! what the C test cannot see: the lock brackets every read and is released
//! on every path, the count is read once, the walk stops at the first active
//! mix, and a raw count above zero answers before the GPU count is read.
use obs_core::video_active::{Mixes, video_active};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Lock,
    Unlock,
    Count,
    At(usize),
    Raw(usize),
    Gpu(usize),
}

#[derive(Default)]
struct Host {
    /// `(raw_active, gpu_encoder_active)` per mix, in list order.
    mixes: Vec<(i64, i64)>,
    locked: bool,
    events: Vec<Event>,
}

impl Host {
    fn new(mixes: &[(i64, i64)]) -> Self {
        Host {
            mixes: mixes.to_vec(),
            ..Host::default()
        }
    }

    /// `obs_video_active()` asked once; the lock must be free again.
    fn ask(&mut self) -> bool {
        self.events.clear();
        let answer = video_active(self);
        assert!(!self.locked, "mixes_mutex left held");
        assert_eq!(self.events.first(), Some(&Event::Lock));
        assert_eq!(self.events.last(), Some(&Event::Unlock));
        assert_eq!(
            self.events
                .iter()
                .filter(|e| matches!(e, Event::Lock | Event::Unlock))
                .count(),
            2
        );
        answer
    }

    /// Asks repeatedly and checks the answer changed no count.
    fn assert_active_is(&mut self, expected: bool) {
        let before = self.mixes.clone();
        for _ in 0..16 {
            assert_eq!(self.ask(), expected);
        }
        assert_eq!(self.mixes, before);
    }
}

impl Mixes for Host {
    type Mix = usize;
    fn lock(&mut self) {
        assert!(!self.locked, "recursive lock");
        self.locked = true;
        self.events.push(Event::Lock);
    }
    fn unlock(&mut self) {
        assert!(self.locked, "unlock without lock");
        self.locked = false;
        self.events.push(Event::Unlock);
    }
    fn count(&mut self) -> usize {
        assert!(self.locked, "count read outside the lock");
        self.events.push(Event::Count);
        self.mixes.len()
    }
    fn at(&mut self, index: usize) -> usize {
        assert!(self.locked, "list read outside the lock");
        assert!(index < self.mixes.len(), "read past the list");
        self.events.push(Event::At(index));
        index
    }
    fn raw_active(&mut self, mix: &usize) -> i64 {
        assert!(self.locked, "raw_active read outside the lock");
        self.events.push(Event::Raw(*mix));
        self.mixes[*mix].0
    }
    fn gpu_encoder_active(&mut self, mix: &usize) -> i64 {
        assert!(self.locked, "gpu_encoder_active read outside the lock");
        self.events.push(Event::Gpu(*mix));
        self.mixes[*mix].1
    }
}

#[test]
fn no_mixes_before_video_reset() {
    let mut host = Host::new(&[]);
    host.assert_active_is(false);
    assert_eq!(
        host.events,
        [Event::Lock, Event::Count, Event::Unlock],
        "an empty list reads only its count"
    );
}

#[test]
fn idle_mix_is_inactive() {
    let mut host = Host::new(&[(0, 0)]);
    host.assert_active_is(false);
}

#[test]
fn raw_callback_on_main_mix() {
    let mut host = Host::new(&[(1, 0)]);
    host.assert_active_is(true);
    // A second consumer keeps it active until both are gone.
    host.mixes[0].0 = 2;
    host.assert_active_is(true);
    host.mixes[0].0 = 1;
    host.assert_active_is(true);
    host.mixes[0].0 = 0;
    host.assert_active_is(false);
}

#[test]
fn auxiliary_canvas_mix_counts() {
    // The main mix first, then an auxiliary mix that has no consumer.
    let mut host = Host::new(&[(0, 0), (0, 0)]);
    host.assert_active_is(false);
    // A raw encoder on the auxiliary canvas's video counts on its mix only.
    host.mixes[1].0 = 3;
    host.assert_active_is(true);
    host.mixes[1].0 = 0;
    host.assert_active_is(false);
}

#[test]
fn gpu_encoder_alone_is_active() {
    let mut host = Host::new(&[(0, 1)]);
    host.assert_active_is(true);
    host.mixes[0].1 = 0;
    host.assert_active_is(false);
}

#[test]
fn only_counts_above_zero_are_active() {
    // Characterized, not endorsed: the check is "above zero", not "nonzero".
    let mut host = Host::new(&[(-1, 0)]);
    host.assert_active_is(false);
    // A negative count does not hide another consumer.
    host.mixes[0].0 = 0;
    host.assert_active_is(false);
    host.mixes[0].0 = 1;
    host.assert_active_is(true);
    host.mixes[0].0 = -1;
    host.assert_active_is(false);
    host.mixes[0].0 = 0;
    host.assert_active_is(false);
    // The same boundary holds for the GPU count and at the extremes.
    for (raw, gpu, expected) in [
        (0, -1, false),
        (i64::MIN, i64::MIN, false),
        (i64::MIN, 1, true),
        (i64::MAX, i64::MIN, true),
        (-1, i64::MAX, true),
    ] {
        host.mixes[0] = (raw, gpu);
        host.assert_active_is(expected);
    }
}

#[test]
fn walk_stops_at_the_first_active_mix() {
    let mut host = Host::new(&[(0, 0), (0, 2), (5, 5), (1, 0)]);
    assert!(host.ask());
    assert_eq!(
        host.events,
        [
            Event::Lock,
            Event::Count,
            Event::At(0),
            Event::Raw(0),
            Event::Gpu(0),
            Event::At(1),
            Event::Raw(1),
            Event::Gpu(1),
            Event::Unlock,
        ]
    );
}

#[test]
fn raw_count_answers_before_the_gpu_count_is_read() {
    let mut host = Host::new(&[(0, -4), (7, 0)]);
    assert!(host.ask());
    assert_eq!(
        host.events,
        [
            Event::Lock,
            Event::Count,
            Event::At(0),
            Event::Raw(0),
            Event::Gpu(0),
            Event::At(1),
            Event::Raw(1),
            Event::Unlock,
        ]
    );
}

#[test]
fn inactive_list_reads_every_mix_once_in_order() {
    let mut host = Host::new(&[(0, 0), (-3, 0), (0, -9)]);
    assert!(!host.ask());
    let mut expected = vec![Event::Lock, Event::Count];
    for i in 0..3 {
        expected.extend([Event::At(i), Event::Raw(i), Event::Gpu(i)]);
    }
    expected.push(Event::Unlock);
    assert_eq!(host.events, expected);
}

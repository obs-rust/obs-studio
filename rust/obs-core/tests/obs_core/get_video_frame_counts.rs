//! Safe mirrors of test_get_video_frame_counts.c over a recording host: the
//! main mix, the mix list, its mutex, the fields the mix rule asks about and
//! each mix's two video_t counters. Plus what the C test cannot see: no lock
//! without a main mix, the lock brackets every list read and is released
//! once, the count is read before every step as the C loop does, each field
//! is read only when the rule asks for it, a mix without a video is skipped,
//! and the sums wrap at `uint32_t`.
use obs_core::get_video_frame_counts::{FrameCounts, get_video_frame_counts, write_counts};

const SENTINEL: u32 = 0xdead_beef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    MainMix,
    Lock,
    Unlock,
    Count,
    At(usize),
    EncoderOnly(usize),
    SameView(usize, usize),
    HasVideo(usize),
    Total(usize),
    Skipped(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mix {
    encoder_only: bool,
    view: u8,
    /// `(total, skipped)` of the mix's video_t; `None` for no video.
    video: Option<(u32, u32)>,
}

const MAIN_VIEW: u8 = 1;
const OTHER_VIEW: u8 = 2;

fn main_mix(total: u32, skipped: u32) -> Mix {
    Mix {
        encoder_only: false,
        view: MAIN_VIEW,
        video: Some((total, skipped)),
    }
}

fn encoder_only(view: u8, total: u32, skipped: u32) -> Mix {
    Mix {
        encoder_only: true,
        view,
        video: Some((total, skipped)),
    }
}

fn canvas_mix(view: u8, total: u32, skipped: u32) -> Mix {
    Mix {
        encoder_only: false,
        view,
        video: Some((total, skipped)),
    }
}

#[derive(Default)]
struct Host {
    /// Every mix, by index; the list and the main mix name them.
    pool: Vec<Mix>,
    list: Vec<usize>,
    main: Option<usize>,
    locked: bool,
    events: Vec<Event>,
}

impl Host {
    fn new(pool: &[Mix], list: &[usize], main: Option<usize>) -> Self {
        Host {
            pool: pool.to_vec(),
            list: list.to_vec(),
            main,
            ..Host::default()
        }
    }

    /// `obs_get_video_frame_counts()` asked once; the lock must be free
    /// again and taken at most once.
    fn ask(&mut self) -> (u32, u32) {
        self.events.clear();
        let counts = get_video_frame_counts(self);
        assert!(!self.locked, "mixes_mutex left held");
        assert_eq!(self.events.first(), Some(&Event::MainMix));
        let locks = self.events.iter().filter(|e| **e == Event::Lock).count();
        let unlocks = self.events.iter().filter(|e| **e == Event::Unlock).count();
        assert_eq!(locks, unlocks);
        if self.main.is_some() {
            assert_eq!(locks, 1);
            assert_eq!(self.events.get(1), Some(&Event::Lock));
            assert_eq!(self.events.last(), Some(&Event::Unlock));
        } else {
            assert_eq!(self.events, [Event::MainMix], "no main mix takes no lock");
        }
        counts
    }

    /// The C test's `frame_counts()`: both outputs start as a sentinel.
    fn frame_counts(&mut self) -> (u32, u32) {
        let (mut total, mut skipped) = (SENTINEL, SENTINEL);
        let counts = self.ask();
        write_counts(counts, Some(&mut total), Some(&mut skipped));
        (total, skipped)
    }

    /// Asks repeatedly and checks the answer changed no mix or list.
    fn assert_frame_counts_are(&mut self, expected: (u32, u32)) {
        let (pool, list, main) = (self.pool.clone(), self.list.clone(), self.main);
        for _ in 0..16 {
            assert_eq!(self.frame_counts(), expected);
        }
        assert_eq!(self.pool, pool);
        assert_eq!(self.list, list);
        assert_eq!(self.main, main);
    }

    fn mix(&self, mix: usize) -> Mix {
        self.pool[mix]
    }
}

impl FrameCounts for Host {
    type Mix = usize;
    fn main_mix(&mut self) -> Option<usize> {
        assert!(!self.locked, "main mix read under the lock");
        self.events.push(Event::MainMix);
        self.main
    }
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
        self.list.len()
    }
    fn at(&mut self, index: usize) -> usize {
        assert!(self.locked, "list read outside the lock");
        assert!(index < self.list.len(), "read past the list");
        self.events.push(Event::At(index));
        self.list[index]
    }
    fn encoder_only(&mut self, mix: &usize) -> bool {
        assert!(self.locked, "encoder_only_mix read outside the lock");
        self.events.push(Event::EncoderOnly(*mix));
        self.mix(*mix).encoder_only
    }
    fn same_view(&mut self, mix: &usize, main: &usize) -> bool {
        assert!(self.locked, "view read outside the lock");
        assert_eq!(Some(*main), self.main, "view compared with another mix");
        self.events.push(Event::SameView(*mix, *main));
        self.mix(*mix).view == self.mix(*main).view
    }
    fn has_video(&mut self, mix: &usize) -> bool {
        assert!(self.locked, "video read outside the lock");
        self.events.push(Event::HasVideo(*mix));
        self.mix(*mix).video.is_some()
    }
    fn total_frames(&mut self, mix: &usize) -> u32 {
        assert!(self.locked, "total frames read outside the lock");
        self.events.push(Event::Total(*mix));
        self.mix(*mix)
            .video
            .expect("counter of a mix without video")
            .0
    }
    fn skipped_frames(&mut self, mix: &usize) -> u32 {
        assert!(self.locked, "skipped frames read outside the lock");
        self.events.push(Event::Skipped(*mix));
        self.mix(*mix)
            .video
            .expect("counter of a mix without video")
            .1
    }
}

/// No main mix yet: both outputs are written as 0, either may be NULL.
#[test]
fn zero_before_video_reset() {
    // The canvas exists, its mix does not; the list may hold other mixes.
    for list in [&[][..], &[0]] {
        let mut host = Host::new(&[canvas_mix(OTHER_VIEW, 9, 4)], list, None);
        host.assert_frame_counts_are((0, 0));

        let counts = host.ask();
        let mut total = SENTINEL;
        write_counts(counts, Some(&mut total), None);
        assert_eq!(total, 0);
        let mut skipped = SENTINEL;
        write_counts(host.ask(), None, Some(&mut skipped));
        assert_eq!(skipped, 0);
        write_counts(host.ask(), None, None);
        assert_eq!(host.main, None);
    }
}

/// After a reset the main mix's video_t counts are reported.
#[test]
fn main_mix_counts() {
    // An idle main mix has counted nothing.
    let mut host = Host::new(&[main_mix(0, 0)], &[0], Some(0));
    host.assert_frame_counts_are((0, 0));

    host.pool[0].video = Some((120, 37));
    host.assert_frame_counts_are((120, 37));
    assert_eq!(host.mix(0).video, Some((120, 37)));
}

/// A NULL output is skipped and the other one is still written.
#[test]
fn null_output_accepted() {
    let mut host = Host::new(&[main_mix(57, 11)], &[0], Some(0));
    for _ in 0..16 {
        let mut total = SENTINEL;
        write_counts(host.ask(), Some(&mut total), None);
        assert_eq!(total, 57);
        let mut skipped = SENTINEL;
        write_counts(host.ask(), None, Some(&mut skipped));
        assert_eq!(skipped, 11);
        write_counts(host.ask(), None, None);
    }
    host.assert_frame_counts_are((57, 11));
}

/// A GPU-rescaling encoder on the main video gets an encoder-only mix of
/// the main view; its video_t's counts are added to the main mix's.
#[test]
fn encoder_only_mix_of_main_view_adds() {
    let mut host = Host::new(
        &[main_mix(57, 11), encoder_only(MAIN_VIEW, 30, 8)],
        &[0, 1],
        Some(0),
    );
    host.assert_frame_counts_are((87, 19));
    // The rescaled mix keeps counting; each answer follows it.
    host.pool[1].video = Some((45, 12));
    host.assert_frame_counts_are((102, 23));
    assert_eq!(host.mix(0).video, Some((57, 11)));

    // Its last encoder stopped, the encoder-only mix is gone: only the main
    // mix counts again.
    host.list.pop();
    host.assert_frame_counts_are((57, 11));
}

/// An auxiliary canvas's mix, and an encoder-only mix of its view, do not
/// count, while they count frames of their own.
#[test]
fn other_view_does_not_count() {
    let mut host = Host::new(
        &[
            main_mix(57, 11),
            canvas_mix(OTHER_VIEW, 200, 90),
            encoder_only(OTHER_VIEW, 150, 70),
        ],
        &[0, 1, 2],
        Some(0),
    );
    host.assert_frame_counts_are((57, 11));
    // Removing the auxiliary canvas and its encoder changes nothing.
    host.list.truncate(1);
    host.assert_frame_counts_are((57, 11));
}

/// Each mix is asked in the C's order and each field read only when the
/// rule asks for it; the count is read before every step.
#[test]
fn fields_are_read_only_when_the_rule_asks() {
    let mut host = Host::new(
        &[
            main_mix(1, 2),
            canvas_mix(MAIN_VIEW, 100, 100),
            encoder_only(OTHER_VIEW, 100, 100),
            Mix {
                video: None,
                ..encoder_only(MAIN_VIEW, 0, 0)
            },
            encoder_only(MAIN_VIEW, 3, 4),
        ],
        &[0, 1, 2, 3, 4],
        Some(0),
    );
    assert_eq!(host.ask(), (4, 6));
    use Event::*;
    assert_eq!(
        host.events,
        [
            MainMix,
            Lock,
            Count,
            At(0),
            HasVideo(0),
            Total(0),
            Skipped(0),
            Count,
            At(1),
            EncoderOnly(1),
            Count,
            At(2),
            EncoderOnly(2),
            SameView(2, 0),
            Count,
            At(3),
            EncoderOnly(3),
            SameView(3, 0),
            HasVideo(3),
            Count,
            At(4),
            EncoderOnly(4),
            SameView(4, 0),
            HasVideo(4),
            Total(4),
            Skipped(4),
            Count,
            Unlock,
        ]
    );
}

/// An empty list reads only its count; a main mix that is not listed, or
/// has no video, counts nothing.
#[test]
fn main_mix_is_counted_only_when_listed_with_video() {
    let mut host = Host::new(&[main_mix(5, 6)], &[], Some(0));
    assert_eq!(host.ask(), (0, 0));
    assert_eq!(
        host.events,
        [Event::MainMix, Event::Lock, Event::Count, Event::Unlock]
    );

    host.list = vec![0];
    host.pool[0].video = None;
    host.assert_frame_counts_are((0, 0));
}

/// A mix listed twice is counted twice, as the C loop does.
#[test]
fn duplicate_entries_count_twice() {
    let mut host = Host::new(
        &[main_mix(5, 6), encoder_only(MAIN_VIEW, 1, 1)],
        &[0, 1, 0, 1],
        Some(0),
    );
    host.assert_frame_counts_are((12, 14));
}

/// The sums wrap at `uint32_t`.
#[test]
fn sums_wrap() {
    let mut host = Host::new(
        &[
            main_mix(u32::MAX, u32::MAX - 1),
            encoder_only(MAIN_VIEW, 2, 3),
            encoder_only(MAIN_VIEW, 5, 0),
        ],
        &[0, 1, 2],
        Some(0),
    );
    host.assert_frame_counts_are((6, 1));
}

/// Outputs are written only when present, total first.
#[test]
fn write_counts_skips_missing_outputs() {
    let (mut total, mut skipped) = (SENTINEL, SENTINEL);
    write_counts((7, 9), Some(&mut total), Some(&mut skipped));
    assert_eq!((total, skipped), (7, 9));
    let (mut total, skipped) = (SENTINEL, SENTINEL);
    write_counts((7, 9), Some(&mut total), None);
    assert_eq!((total, skipped), (7, SENTINEL));
    let (total, mut skipped) = (SENTINEL, SENTINEL);
    write_counts((7, 9), None, Some(&mut skipped));
    assert_eq!((total, skipped), (SENTINEL, 9));
    write_counts((7, 9), None, None);
}

//! Tier 1 mirrors test_canvas_reset_video_entry and the reset rules of
//! test_canvas_rules_callers: NULL info refused before any canvas read,
//! the main canvas and active video refused without a reset, a removed
//! canvas still reset, and the internal reset's result forwarded as is.
use obs_core::canvas_reset_video::{Host, reset_video};
use obs_core::canvas_rules::{MAIN, PREVIEW, PROGRAM, REMOVED};

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Flags(usize),
    Active,
    Reset(usize, u32),
}

struct Fixture {
    flags: [u32; 2],
    active: bool,
    result: bool,
    events: Vec<Event>,
}
impl Fixture {
    fn new(flags: u32, active: bool, result: bool) -> Self {
        Self {
            flags: [flags, MAIN],
            active,
            result,
            events: Vec::new(),
        }
    }
}
impl Host for Fixture {
    type Canvas = usize;
    type Info = u32;
    fn flags(&mut self, canvas: usize) -> u32 {
        self.events.push(Event::Flags(canvas));
        self.flags[canvas]
    }
    fn video_active(&mut self) -> bool {
        self.events.push(Event::Active);
        self.active
    }
    fn reset_internal(&mut self, canvas: usize, info: u32) -> bool {
        self.events.push(Event::Reset(canvas, info));
        self.result
    }
}

#[test]
fn null_info_first() {
    let mut f = Fixture::new(PROGRAM, false, true);
    assert!(!reset_video(&mut f, 0, None));
    assert!(f.events.is_empty(), "no canvas read or core query");
}

#[test]
fn removed_canvas_resets() {
    let mut f = Fixture::new(PROGRAM | REMOVED, false, true);
    assert!(reset_video(&mut f, 0, Some(7)));
    assert_eq!(
        f.events,
        [Event::Flags(0), Event::Active, Event::Reset(0, 7)]
    );
}

#[test]
fn main_canvas_untouched() {
    let mut f = Fixture::new(MAIN | PROGRAM, false, true);
    assert!(!reset_video(&mut f, 0, Some(7)));
    assert_eq!(f.events, [Event::Flags(0)], "video activity not asked");
    let mut f = Fixture::new(PROGRAM, false, true);
    assert!(!reset_video(&mut f, 1, Some(7)));
    assert_eq!(f.events, [Event::Flags(1)]);
}

#[test]
fn active_video_refused() {
    let mut f = Fixture::new(PREVIEW, true, true);
    assert!(!reset_video(&mut f, 0, Some(3)));
    assert_eq!(f.events, [Event::Flags(0), Event::Active]);
}

#[test]
fn failed_mix_result_forwarded() {
    for result in [false, true] {
        let mut f = Fixture::new(PROGRAM, false, result);
        assert_eq!(reset_video(&mut f, 0, Some(9)), result);
        assert_eq!(
            f.events,
            [Event::Flags(0), Event::Active, Event::Reset(0, 9)]
        );
    }
}

#[test]
fn every_flag_word_follows_the_rule() {
    for bits in 0..64u32 {
        let flags = bits | (bits << 26);
        for active in [false, true] {
            let mut f = Fixture::new(flags, active, true);
            let expect = flags & MAIN == 0 && !active;
            assert_eq!(
                reset_video(&mut f, 0, Some(1)),
                expect,
                "{flags:x} {active}"
            );
        }
    }
}

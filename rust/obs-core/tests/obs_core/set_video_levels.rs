//! Safe mirrors of test_set_video_levels.c and test_video_levels_range.c over
//! a host core with the levels `obs_reset_video()` leaves (300 and 1000), a
//! graphics subsystem and a warning log. Plus what the C tests cannot see:
//! with no core one warning and nothing else is read or written, with no
//! graphics nothing at all, the stored level is read only for a rejected
//! level's warning, and the SDR white level is handled before the HDR
//! nominal peak level.
use obs_core::set_video_levels::{Levels, set_video_levels};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Event {
    Core,
    Graphics,
    ReadSdr,
    SetSdr(u32),
    ReadHdr,
    SetHdr(u32),
    WarnNoCore,
    /// Rejected and kept, as bits.
    WarnSdr(u32, u32),
    WarnHdr(u32, u32),
}

/// The core's video fields the setter reaches.
#[derive(Clone, Copy)]
struct Video {
    graphics: bool,
    sdr: f32,
    hdr: f32,
}

struct Host {
    video: Option<Video>,
    events: Vec<Event>,
}

impl Host {
    /// A core after `obs_reset_video()`: graphics, 300 and 1000.
    fn live() -> Self {
        Host {
            video: Some(Video {
                graphics: true,
                sdr: 300.0,
                hdr: 1000.0,
            }),
            events: Vec::new(),
        }
    }

    fn set(&mut self, white: f32, peak: f32) {
        set_video_levels(self, white, peak);
    }

    fn levels(&self) -> (u32, u32) {
        let video = self.video.unwrap();
        (video.sdr.to_bits(), video.hdr.to_bits())
    }

    fn video(&mut self) -> &mut Video {
        self.video.as_mut().unwrap()
    }

    /// The warnings logged since the last call, cleared.
    fn warnings(&mut self) -> Vec<Event> {
        let warnings = self
            .events
            .iter()
            .copied()
            .filter(|e| {
                matches!(
                    e,
                    Event::WarnNoCore | Event::WarnSdr(..) | Event::WarnHdr(..)
                )
            })
            .collect();
        self.events.clear();
        warnings
    }

    /// `set_levels_expect()` of test_video_levels_range.c.
    fn set_expect(&mut self, white: f32, peak: f32, want: (f32, f32), warnings: usize) {
        self.events.clear();
        self.set(white, peak);
        assert_eq!(
            self.levels(),
            (want.0.to_bits(), want.1.to_bits()),
            "obs_set_video_levels({white}, {peak})"
        );
        assert_eq!(
            self.warnings().len(),
            warnings,
            "obs_set_video_levels({white}, {peak})"
        );
    }
}

impl Levels for Host {
    fn core(&mut self) -> bool {
        self.events.push(Event::Core);
        self.video.is_some()
    }
    fn graphics(&mut self) -> bool {
        self.events.push(Event::Graphics);
        self.video().graphics
    }
    fn sdr_white_level(&mut self) -> f32 {
        self.events.push(Event::ReadSdr);
        self.video().sdr
    }
    fn set_sdr_white_level(&mut self, level: f32) {
        self.events.push(Event::SetSdr(level.to_bits()));
        self.video().sdr = level;
    }
    fn hdr_nominal_peak_level(&mut self) -> f32 {
        self.events.push(Event::ReadHdr);
        self.video().hdr
    }
    fn set_hdr_nominal_peak_level(&mut self, level: f32) {
        self.events.push(Event::SetHdr(level.to_bits()));
        self.video().hdr = level;
    }
    fn warn_no_core(&mut self) {
        self.events.push(Event::WarnNoCore);
    }
    fn warn_sdr_white_level(&mut self, rejected: f32, kept: f32) {
        self.events
            .push(Event::WarnSdr(rejected.to_bits(), kept.to_bits()));
    }
    fn warn_hdr_nominal_peak_level(&mut self, rejected: f32, kept: f32) {
        self.events
            .push(Event::WarnHdr(rejected.to_bits(), kept.to_bits()));
    }
}

fn sdr(rejected: f32, kept: f32) -> Event {
    Event::WarnSdr(rejected.to_bits(), kept.to_bits())
}

fn hdr(rejected: f32, kept: f32) -> Event {
    Event::WarnHdr(rejected.to_bits(), kept.to_bits())
}

fn bits(white: f32, peak: f32) -> (u32, u32) {
    (white.to_bits(), peak.to_bits())
}

// --- test_set_video_levels.c ----------------------------------------------

#[test]
fn ignored_levels_name_values() {
    let mut host = Host::live();
    host.set(200.0, 600.0);
    assert_eq!(host.levels(), bits(200.0, 600.0));
    assert!(host.warnings().is_empty());

    host.set(-1.0, 700.0);
    assert_eq!(host.levels(), bits(200.0, 700.0));
    assert_eq!(host.warnings(), [sdr(-1.0, 200.0)]);

    host.set(250.0, 0.0);
    assert_eq!(host.levels(), bits(250.0, 700.0));
    assert_eq!(host.warnings(), [hdr(0.0, 700.0)]);

    // Both ignored: one warning each, SDR white first.
    host.set(-2.5, -3.5);
    assert_eq!(host.levels(), bits(250.0, 700.0));
    assert_eq!(host.warnings(), [sdr(-2.5, 250.0), hdr(-3.5, 700.0)]);
}

#[test]
fn valid_calls_log_nothing_and_latest_wins() {
    let mut host = Host::live();
    host.set(100.0, 200.0);
    assert_eq!(host.levels(), bits(100.0, 200.0));
    host.set(400.0, 800.0);
    assert_eq!(host.levels(), bits(400.0, 800.0));
    host.set(400.0, 800.0);
    assert_eq!(host.levels(), bits(400.0, 800.0));
    host.set(80.0, 10000.0);
    assert_eq!(host.levels(), bits(80.0, 10000.0));
    assert!(host.warnings().is_empty());
}

#[test]
fn levels_do_not_survive_restart() {
    // The setter keeps nothing of its own: the levels live in the core, and
    // the next core starts from obs_reset_video()'s defaults.
    let mut first = Host::live();
    first.set(123.0, 456.0);
    assert_eq!(first.levels(), bits(123.0, 456.0));
    let second = Host::live();
    assert_eq!(second.levels(), bits(300.0, 1000.0));
}

// --- test_video_levels_range.c --------------------------------------------

const BAD: [f32; 6] = [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY];

#[test]
fn defaults_before_any_set() {
    let host = Host::live();
    assert_eq!(host.levels(), bits(300.0, 1000.0));
    assert!(host.events.is_empty());
}

#[test]
fn bad_white_keeps_white() {
    let mut host = Host::live();
    for bad in BAD {
        host.set_expect(200.0, 600.0, (200.0, 600.0), 0);
        host.set_expect(bad, 700.0, (200.0, 700.0), 1);
    }
}

#[test]
fn bad_peak_keeps_peak() {
    let mut host = Host::live();
    for bad in BAD {
        host.set_expect(200.0, 600.0, (200.0, 600.0), 0);
        host.set_expect(250.0, bad, (250.0, 600.0), 1);
    }
}

#[test]
fn bad_pair_keeps_both() {
    let mut host = Host::live();
    for white in BAD {
        for peak in BAD {
            host.set_expect(200.0, 600.0, (200.0, 600.0), 0);
            host.set_expect(white, peak, (200.0, 600.0), 2);
        }
    }
}

#[test]
fn valid_pairs_stored_exactly() {
    let mut host = Host::live();
    for (white, peak) in [
        (300.0, 1000.0),
        (80.0, 80.0),
        (1.0, 10000.0),
        (480.0, 400.0),
        (f32::MAX, f32::MAX),
        (f32::MIN_POSITIVE, f32::MIN_POSITIVE),
        (f32::MAX, 1000.0),
        (300.0, f32::MAX),
        (4_294_967_295.0, 4_294_967_295.0),
    ] {
        host.set_expect(white, peak, (white, peak), 0);
    }
    host.set_expect(300.0, 1000.0, (300.0, 1000.0), 0);
}

// --- What the C tests cannot see ------------------------------------------

#[test]
fn no_core_warns_once_and_reads_nothing_else() {
    let mut host = Host {
        video: None,
        events: Vec::new(),
    };
    for (white, peak) in [(200.0, 600.0), (-1.0, f32::NAN)] {
        host.set(white, peak);
        assert_eq!(host.events, [Event::Core, Event::WarnNoCore]);
        host.events.clear();
    }
}

#[test]
fn no_graphics_touches_nothing() {
    let mut host = Host::live();
    host.video().graphics = false;
    for (white, peak) in [(200.0, 600.0), (-1.0, 0.0), (f32::NAN, 700.0)] {
        host.set(white, peak);
        assert_eq!(host.events, [Event::Core, Event::Graphics]);
        assert_eq!(host.levels(), bits(300.0, 1000.0));
        host.events.clear();
    }
}

#[test]
fn reads_only_for_the_warning_and_sdr_first() {
    let mut host = Host::live();
    host.set(200.0, 600.0);
    assert_eq!(
        host.events,
        [
            Event::Core,
            Event::Graphics,
            Event::SetSdr(200.0f32.to_bits()),
            Event::SetHdr(600.0f32.to_bits()),
        ]
    );
    host.events.clear();

    host.set(f32::NAN, f32::INFINITY);
    assert_eq!(
        host.events,
        [
            Event::Core,
            Event::Graphics,
            Event::ReadSdr,
            sdr(f32::NAN, 200.0),
            Event::ReadHdr,
            hdr(f32::INFINITY, 600.0),
        ]
    );
    host.events.clear();

    // Mixed: the stored SDR level is written before the HDR one is read.
    host.set(250.0, -0.0);
    assert_eq!(
        host.events,
        [
            Event::Core,
            Event::Graphics,
            Event::SetSdr(250.0f32.to_bits()),
            Event::ReadHdr,
            hdr(-0.0, 600.0),
        ]
    );
}

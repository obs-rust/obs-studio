//! Tier 1 mirrors of test_source_audio_mixers_getter.c.
use core::cell::Cell;
use obs_source::{audio_mixers_getter::{get, Source}, monitoring::AUDIO};

struct Probe {
    owned: bool,
    flags: u32,
    mixers: Cell<u32>,
    reads: Cell<(usize, usize, usize)>,
}

impl Probe {
    fn new(owned: bool, flags: u32, mixers: u32) -> Self {
        Self { owned, flags, mixers: Cell::new(mixers), reads: Cell::new((0, 0, 0)) }
    }
    fn get(&self) -> u32 {
        get(Some(self), |_, _| panic!("valid source must not log"))
    }
}
impl Source for Probe {
    fn owns_info_id(&self) -> bool {
        let (o, f, m) = self.reads.get();
        self.reads.set((o + 1, f, m));
        self.owned
    }
    fn output_flags(&self) -> u32 {
        let (o, f, m) = self.reads.get();
        self.reads.set((o, f + 1, m));
        self.flags
    }
    fn audio_mixers(&self) -> u32 {
        let (o, f, m) = self.reads.get();
        self.reads.set((o, f, m + 1));
        self.mixers.get()
    }
}

#[test]
fn test_source_audio_mixers_getter_null_source() {
    let mut logs = Vec::new();
    assert_eq!(get(None::<&Probe>, |level, message| logs.push((level, message))), 0);
    assert_eq!(logs, [(400, c"obs_source_get_audio_mixers: Null 'source' parameter")]);
}

#[test]
fn test_source_audio_mixers_getter_capability_gate() {
    // VIDEO, COMPOSITE, VIDEO | ASYNC from the unchanged C test.
    for flags in [0, 1, 1 << 6, 1 | (1 << 2)] {
        let source = Probe::new(false, flags, u32::MAX);
        assert_eq!(source.get(), 0);
        assert_eq!(source.reads.get(), (1, 1, 0));
    }
    assert_eq!(Probe::new(false, AUDIO, u32::MAX).get(), u32::MAX);
    // Owned IDs bypass the capability gate, characterized, not endorsed.
    let source = Probe::new(true, 0, u32::MAX);
    assert_eq!(source.get(), u32::MAX);
    assert_eq!(source.reads.get(), (1, 0, 1));
}

#[test]
fn test_source_audio_mixers_getter_setter_round_trip() {
    let audio = Probe::new(false, AUDIO, 0xFF);
    let silent = Probe::new(false, 0, 0xFF);
    assert_eq!(audio.get(), 0xFF);
    assert_eq!(silent.get(), 0);
    for mask in [0, 1, 0x3F, 0x8000_0000, u32::MAX, 0xA5A5_5A5A, 0] {
        audio.mixers.set(mask);
        silent.mixers.set(mask);
        assert_eq!(audio.get(), mask);
        assert_eq!(silent.get(), 0);
        assert_eq!(audio.mixers.get(), mask);
        assert_eq!(silent.mixers.get(), mask);
    }
}

#[test]
fn every_flag_bit_and_owned_bypass() {
    for bit in 0..32 {
        let flags = 1 << bit;
        assert_eq!(Probe::new(false, flags, 0x8000_0001).get(), if flags == AUDIO { 0x8000_0001 } else { 0 });
        assert_eq!(Probe::new(true, flags, 0x8000_0001).get(), 0x8000_0001);
        assert_eq!(Probe::new(false, flags | AUDIO, 0x8000_0001).get(), 0x8000_0001);
    }
}

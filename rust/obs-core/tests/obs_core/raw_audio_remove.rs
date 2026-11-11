//! Tier 1: every membership assertion of test_raw_audio_remove.c, using safe
//! identities and a synchronous output model. No audio threads or timing waits.
use obs_core::raw_audio_remove::{Audio, remove};

const MIXES: usize = 6;
type Identity = Option<u8>;
type Tuple = (Identity, Identity);
type Call = (Option<usize>, usize, Identity, Identity);

#[derive(Default)]
struct Outputs {
    main: Option<usize>,
    mixes: [Vec<Tuple>; MIXES],
    independent: Vec<Tuple>,
    calls: Vec<Call>,
    reads: usize,
}

impl Audio for Outputs {
    type Output = Option<usize>;
    type Callback = Identity;
    type Param = Identity;

    fn main_audio(&mut self) -> Self::Output {
        self.reads += 1;
        self.main
    }

    fn disconnect(&mut self, output: Self::Output, mix: usize, cb: Identity, param: Identity) {
        self.calls.push((output, mix, cb, param));
        if output == Some(7)
            && let Some(list) = self.mixes.get_mut(mix)
            && let Some(i) = list.iter().position(|&entry| entry == (cb, param))
        {
            list.remove(i);
        }
    }
}

impl Outputs {
    fn open() -> Self {
        let out = Self {
            main: Some(7),
            ..Self::default()
        };
        assert!(!out.active());
        out
    }
    fn connect(&mut self, mix: usize, cb: Identity, param: Identity) -> bool {
        let list = &mut self.mixes[mix];
        if list.contains(&(cb, param)) {
            return false;
        }
        list.push((cb, param));
        true
    }
    fn present(&mut self, mix: usize, cb: Identity, param: Identity) {
        assert!(!self.connect(mix, cb, param));
    }
    fn absent(&mut self, mix: usize, cb: Identity, param: Identity) {
        assert!(self.connect(mix, cb, param));
        // Direct output cleanup, independent of the wrapper under test.
        self.disconnect(self.main, mix, cb, param);
    }
    fn active(&self) -> bool {
        self.mixes.iter().any(|list| !list.is_empty())
    }
    fn remove(&mut self, mix: usize, cb: Identity, param: Identity) {
        let calls = self.calls.len();
        let reads = self.reads;
        remove(self, mix, cb, param);
        assert_eq!(self.reads, reads + 1);
        assert_eq!(self.calls.len(), calls + 1);
        assert_eq!(self.calls[calls], (self.main, mix, cb, param));
    }
}

#[test]
fn test_raw_audio_remove_initialized_core_without_audio() {
    let mut out = Outputs::default();
    assert_eq!(out.main, None);
    out.remove(0, Some(1), Some(1));
    out.remove(MIXES, Some(2), None);
    out.remove(usize::MAX, None, None);
    assert_eq!(out.main, None);
}

#[test]
fn test_raw_audio_remove_matching_mix_and_other_mixes() {
    let mut out = Outputs::open();
    for mix in 0..MIXES {
        assert!(out.connect(mix, Some(1), Some(1)));
    }
    for mix in 0..MIXES {
        out.remove(mix, Some(1), Some(1));
        out.absent(mix, Some(1), Some(1));
        out.remove(mix, Some(1), Some(1));
        for remaining in mix + 1..MIXES {
            out.present(remaining, Some(1), Some(1));
        }
        assert_eq!(out.active(), mix + 1 < MIXES);
    }
}

#[test]
fn test_raw_audio_remove_callback_and_parameter_identity() {
    let mut out = Outputs::open();
    let entries = [
        (Some(1), Some(1)),
        (Some(1), Some(2)),
        (Some(2), Some(1)),
        (Some(2), None),
    ];
    for (cb, param) in entries {
        assert!(out.connect(0, cb, param));
    }
    for (cb, param) in [
        (Some(2), Some(2)),
        (Some(1), Some(3)),
        (Some(3), Some(1)),
        (None, Some(1)),
    ] {
        out.remove(0, cb, param);
    }
    out.remove(1, Some(1), Some(1));
    for (cb, param) in entries {
        out.present(0, cb, param);
    }
    out.remove(0, Some(1), Some(2));
    out.absent(0, Some(1), Some(2));
    for (cb, param) in [entries[0], entries[2], entries[3]] {
        out.present(0, cb, param);
    }
    out.remove(0, Some(1), Some(1));
    out.absent(0, Some(1), Some(1));
    for (cb, param) in [entries[2], entries[3]] {
        out.present(0, cb, param);
    }
    out.remove(0, Some(2), None);
    out.absent(0, Some(2), None);
    out.present(0, Some(2), Some(1));
    assert!(out.active());
    out.remove(0, Some(2), Some(1));
    out.absent(0, Some(2), Some(1));
    assert!(!out.active());
}

#[test]
fn test_raw_audio_remove_invalid_full_size_mix() {
    let mut out = Outputs::open();
    for mix in 0..MIXES {
        assert!(out.connect(mix, Some(1), Some(1)));
    }
    let mut invalid = vec![MIXES, MIXES + 1, usize::MAX];
    if let Some(wide) = usize::try_from(u32::MAX).unwrap().checked_add(1) {
        invalid.extend([wide, wide + 1]);
    }
    for mix in invalid {
        out.remove(mix, Some(1), Some(1));
    }
    for mix in 0..MIXES {
        out.present(mix, Some(1), Some(1));
    }
}

#[test]
fn test_raw_audio_remove_delegates_to_main_audio() {
    let mut out = Outputs::open();
    out.independent.push((Some(1), Some(1)));
    assert!(out.connect(0, Some(1), Some(1)));
    out.present(0, Some(1), Some(1));
    out.remove(0, Some(1), Some(1));
    out.absent(0, Some(1), Some(1));
    assert!(!out.active());
    assert_eq!(out.independent, [(Some(1), Some(1))]);
    assert!(out.connect(MIXES - 1, Some(2), Some(2)));
    out.remove(MIXES - 1, Some(2), Some(2));
    out.absent(MIXES - 1, Some(2), Some(2));
    assert!(!out.active());
    assert_eq!(out.independent, [(Some(1), Some(1))]);
    assert_eq!(out.main, Some(7));
}

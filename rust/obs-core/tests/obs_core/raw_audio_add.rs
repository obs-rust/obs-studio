//! Tier 1: every registration assertion of test_raw_audio_add.c, using safe
//! identities and a synchronous output model. No audio threads or timing waits.
use obs_core::raw_audio_add::{Audio, add};

const MIXES: usize = 6;
/// Output ids of the model: the core's first output, its replacement, and an
/// output the core does not own.
const MAIN: usize = 7;
const REOPENED: usize = 8;
const INDEPENDENT: usize = 9;
type Identity = Option<u8>;
type Conversion = Option<u32>;
type Tuple = (Identity, Identity);
type Call = (Option<usize>, usize, Conversion, Identity, Identity);

#[derive(Default)]
struct Outputs {
    main: Option<usize>,
    lists: Vec<(usize, [Vec<Tuple>; MIXES])>,
    calls: Vec<Call>,
    reads: usize,
}

impl Audio for Outputs {
    type Output = Option<usize>;
    type Conversion = Conversion;
    type Callback = Identity;
    type Param = Identity;

    fn main_audio(&mut self) -> Self::Output {
        self.reads += 1;
        self.main
    }

    fn connect(
        &mut self,
        output: Self::Output,
        mix: usize,
        conversion: Conversion,
        cb: Identity,
        param: Identity,
    ) {
        self.calls.push((output, mix, conversion, cb, param));
        if let Some(output) = output {
            // The media-io result is ignored by the wrapper, as in the C.
            let _ = self.link(output, mix, cb, param);
        }
    }
}

impl Outputs {
    /// `open_main_audio`: a fresh, inactive main output.
    fn open(id: usize) -> Self {
        let mut out = Self::default();
        out.reopen(id);
        out
    }
    fn reopen(&mut self, id: usize) {
        self.main = Some(id);
        self.lists.push((id, Default::default()));
        assert!(!self.active(id));
    }
    fn mixes(&mut self, output: usize) -> &mut [Vec<Tuple>; MIXES] {
        if !self.lists.iter().any(|(id, _)| *id == output) {
            self.lists.push((output, Default::default()));
        }
        let at = self.lists.iter().position(|(id, _)| *id == output).unwrap();
        &mut self.lists[at].1
    }
    /// `audio_output_connect`: an out-of-range mix and a duplicate are refused.
    fn link(&mut self, output: usize, mix: usize, cb: Identity, param: Identity) -> bool {
        let Some(list) = self.mixes(output).get_mut(mix) else {
            return false;
        };
        if list.contains(&(cb, param)) {
            return false;
        }
        list.push((cb, param));
        true
    }
    /// `audio_output_disconnect`, through the direct API.
    fn unlink(&mut self, output: usize, mix: usize, cb: Identity, param: Identity) {
        if let Some(list) = self.mixes(output).get_mut(mix)
            && let Some(i) = list.iter().position(|&entry| entry == (cb, param))
        {
            list.remove(i);
        }
    }
    /// `obs_remove_raw_audio_callback`, as the cmocka test uses it.
    fn remove(&mut self, mix: usize, cb: Identity, param: Identity) {
        if let Some(main) = self.main {
            self.unlink(main, mix, cb, param);
        }
    }
    fn present(&mut self, output: usize, mix: usize, cb: Identity, param: Identity) {
        assert!(!self.link(output, mix, cb, param));
    }
    fn absent(&mut self, output: usize, mix: usize, cb: Identity, param: Identity) {
        assert!(self.link(output, mix, cb, param));
        // Direct output cleanup, independent of the wrapper under test.
        self.unlink(output, mix, cb, param);
    }
    fn active(&mut self, output: usize) -> bool {
        self.mixes(output).iter().any(|list| !list.is_empty())
    }
    fn add(&mut self, mix: usize, conversion: Conversion, cb: Identity, param: Identity) {
        let calls = self.calls.len();
        let reads = self.reads;
        add(self, mix, conversion, cb, param);
        assert_eq!(self.reads, reads + 1);
        assert_eq!(self.calls.len(), calls + 1);
        assert_eq!(self.calls[calls], (self.main, mix, conversion, cb, param));
    }
}

#[test]
fn test_raw_audio_add_initialized_core_without_audio() {
    let mut out = Outputs::default();
    out.add(0, None, Some(1), Some(1));
    out.add(MIXES, None, Some(2), None);
    out.add(usize::MAX, None, None, None);
    assert_eq!(out.main, None);

    // Nothing was kept for later: audio set up afterwards starts empty.
    out.reopen(MAIN);
    out.absent(MAIN, 0, Some(1), Some(1));
    assert!(!out.active(MAIN));
}

#[test]
fn test_raw_audio_add_every_mix_registers() {
    let mut out = Outputs::open(MAIN);
    for mix in 0..MIXES {
        out.add(mix, None, Some(1), Some(1));
        out.present(MAIN, mix, Some(1), Some(1));
        assert!(out.active(MAIN));
        for later in mix + 1..MIXES {
            out.absent(MAIN, later, Some(1), Some(1));
        }
    }
    for mix in 0..MIXES {
        out.remove(mix, Some(1), Some(1));
        out.absent(MAIN, mix, Some(1), Some(1));
    }
    assert!(!out.active(MAIN));
}

#[test]
fn test_raw_audio_add_repeated_add_is_one_registration() {
    let mut out = Outputs::open(MAIN);
    out.add(0, None, Some(1), Some(1));
    out.add(0, None, Some(1), Some(1));
    out.present(MAIN, 0, Some(1), Some(1));
    out.remove(0, Some(1), Some(1));
    out.absent(MAIN, 0, Some(1), Some(1));
    assert!(!out.active(MAIN));
}

#[test]
fn test_raw_audio_add_callback_and_parameter_identity() {
    let mut out = Outputs::open(MAIN);
    let entries = [
        (Some(1), Some(1)),
        (Some(1), Some(2)),
        (Some(2), Some(1)),
        (Some(2), None),
    ];
    for (cb, param) in entries {
        out.add(0, None, cb, param);
    }
    for (cb, param) in entries {
        out.present(MAIN, 0, cb, param);
    }
    out.absent(MAIN, 0, Some(2), Some(2));
    out.absent(MAIN, 1, Some(1), Some(1));

    out.remove(0, Some(1), Some(2));
    for (cb, param) in [entries[0], entries[2], entries[3]] {
        out.present(MAIN, 0, cb, param);
    }
}

#[test]
fn test_raw_audio_add_conversion_is_accepted() {
    let mut out = Outputs::open(MAIN);
    out.add(1, Some(44_100), Some(2), Some(2));
    out.present(MAIN, 1, Some(2), Some(2));
    // A later add of the same tuple with no conversion is still a duplicate.
    out.add(1, None, Some(2), Some(2));
    out.remove(1, Some(2), Some(2));
    out.absent(MAIN, 1, Some(2), Some(2));
    assert!(!out.active(MAIN));
}

#[test]
fn test_raw_audio_add_invalid_full_size_mix() {
    let mut out = Outputs::open(MAIN);
    let mut invalid = vec![MIXES, MIXES + 1, usize::MAX];
    // On wide hosts these would alias mixes 0 and 1 if the index were narrowed.
    if let Some(wide) = usize::try_from(u32::MAX).unwrap().checked_add(1) {
        invalid.extend([wide, wide + 1]);
    }
    for mix in invalid {
        out.add(mix, None, Some(1), Some(1));
    }
    assert!(!out.active(MAIN));
    for mix in 0..MIXES {
        out.absent(MAIN, mix, Some(1), Some(1));
    }
}

#[test]
fn test_raw_audio_add_connects_to_main_audio() {
    let mut out = Outputs::open(MAIN);
    assert!(!out.active(INDEPENDENT));

    out.add(MIXES - 1, None, Some(1), Some(1));
    out.present(MAIN, MIXES - 1, Some(1), Some(1));
    assert!(out.active(MAIN));
    // An output the core does not own is left alone.
    assert!(!out.active(INDEPENDENT));
    out.absent(INDEPENDENT, MIXES - 1, Some(1), Some(1));

    // After the core's audio is replaced, a new add goes to the new output.
    out.unlink(MAIN, MIXES - 1, Some(1), Some(1));
    out.reopen(REOPENED);
    out.add(0, None, Some(2), Some(2));
    out.present(REOPENED, 0, Some(2), Some(2));
    assert!(!out.active(MAIN));
    assert_eq!(out.main, Some(REOPENED));
}

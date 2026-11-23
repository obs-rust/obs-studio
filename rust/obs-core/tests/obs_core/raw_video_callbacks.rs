//! Tier 1: every case of test_raw_video_callbacks.c, using safe identities and
//! a synchronous model of the main canvas, its mix and libobs' start/stop
//! (video-io's connect2/disconnect2 rules and the mix's `raw_active` count).
//! No video threads or timing waits.
use obs_core::raw_video_callbacks::{Video, add, add2, remove};

/// Video ids of the model: the stand-in mix's video and an output the core
/// does not use (the replacement mix's video in the last case).
const MAIN: usize = 1;
const OTHER: usize = 2;
type Identity = Option<u8>;
type Conversion = Option<u32>;
type Pair = (Identity, Identity);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Call {
    Start(usize, Conversion, u32, Identity, Identity),
    Stop(usize, Identity, Identity),
    Warn,
}

/// A mix: its video and its `raw_active` count. The stand-in is not in the
/// core's mix list, so start/stop never count it.
#[derive(Clone, Copy)]
struct Mix {
    video: usize,
    raw_active: i64,
}

struct Core {
    canvas: bool,
    mix: Option<Mix>,
    outputs: Vec<(usize, Vec<Pair>)>,
    calls: Vec<Call>,
}

impl Video for Core {
    type Video = usize;
    type Conversion = Conversion;
    type Callback = Identity;
    type Param = Identity;

    fn main_video(&mut self) -> Option<usize> {
        if !self.canvas {
            return None;
        }
        self.mix.map(|mix| mix.video)
    }

    fn warn_not_set_up(&mut self) {
        self.calls.push(Call::Warn);
    }

    fn start(
        &mut self,
        video: usize,
        conversion: Conversion,
        divisor: u32,
        cb: Identity,
        param: Identity,
    ) {
        self.calls
            .push(Call::Start(video, conversion, divisor, cb, param));
        // start_raw_video counts only a listed mix; the stand-in is not one.
        let _connected = divisor != 0 && self.connect(video, cb, param);
    }

    fn stop(&mut self, video: usize, cb: Identity, param: Identity) {
        self.calls.push(Call::Stop(video, cb, param));
        let _ = self.disconnect(video, cb, param);
    }
}

impl Core {
    /// `setup`: an initialized core whose main canvas mix is the stand-in.
    fn setup() -> Self {
        let mut core = Self {
            canvas: true,
            mix: Some(Mix {
                video: MAIN,
                raw_active: 0,
            }),
            outputs: vec![(MAIN, Vec::new()), (OTHER, Vec::new())],
            calls: Vec::new(),
        };
        assert!(!core.active(MAIN));
        assert!(!core.active(OTHER));
        core
    }
    fn list(&mut self, video: usize) -> &mut Vec<Pair> {
        &mut self
            .outputs
            .iter_mut()
            .find(|(id, _)| *id == video)
            .unwrap()
            .1
    }
    /// `video_output_connect2`: a NULL callback and a duplicate are refused.
    fn connect(&mut self, video: usize, cb: Identity, param: Identity) -> bool {
        let list = self.list(video);
        if cb.is_none() || list.contains(&(cb, param)) {
            return false;
        }
        list.push((cb, param));
        true
    }
    /// `video_output_disconnect2`: false for a missing pair.
    fn disconnect(&mut self, video: usize, cb: Identity, param: Identity) -> bool {
        let list = self.list(video);
        let Some(i) = list.iter().position(|&pair| pair == (cb, param)) else {
            return false;
        };
        list.remove(i);
        true
    }
    fn active(&mut self, video: usize) -> bool {
        !self.list(video).is_empty()
    }
    fn present(&mut self, video: usize, cb: Identity, param: Identity) {
        assert!(!self.connect(video, cb, param));
    }
    fn absent(&mut self, video: usize, cb: Identity, param: Identity) {
        assert!(!self.disconnect(video, cb, param));
    }
    fn assert_uncounted(&self) {
        assert_eq!(self.mix.unwrap().raw_active, 0);
    }
    fn expect(&mut self, from: usize, call: Option<Call>) {
        assert_eq!(&self.calls[from..], call.as_slice());
    }
    fn add(&mut self, conversion: Conversion, cb: Identity, param: Identity) {
        let at = self.calls.len();
        let video = self.main_video();
        add(self, conversion, cb, param);
        self.expect(
            at,
            Some(video.map_or(Call::Warn, |v| Call::Start(v, conversion, 1, cb, param))),
        );
    }
    fn add2(&mut self, conversion: Conversion, divisor: u32, cb: Identity, param: Identity) {
        let at = self.calls.len();
        let video = self.main_video();
        add2(self, conversion, divisor, cb, param);
        self.expect(
            at,
            Some(video.map_or(Call::Warn, |v| {
                Call::Start(v, conversion, divisor, cb, param)
            })),
        );
    }
    fn remove(&mut self, cb: Identity, param: Identity) {
        let at = self.calls.len();
        let video = self.main_video();
        remove(self, cb, param);
        self.expect(at, video.map(|v| Call::Stop(v, cb, param)));
    }
}

const A: Identity = Some(1);
const B: Identity = Some(2);

#[test]
fn test_raw_video_callbacks_add_registers_on_main_mix_video() {
    let mut core = Core::setup();
    core.absent(MAIN, A, A);
    core.add(None, A, A);
    core.present(MAIN, A, A);
    assert!(core.active(MAIN));
    // An output the core does not use is left alone.
    core.absent(OTHER, A, A);
    assert!(!core.active(OTHER));
    core.assert_uncounted();

    core.remove(A, A);
    core.absent(MAIN, A, A);
    assert!(!core.active(MAIN));
    core.assert_uncounted();
}

#[test]
fn test_raw_video_callbacks_add2_registers_with_any_nonzero_divisor() {
    let mut core = Core::setup();
    core.add2(None, 1, A, A);
    core.add2(None, 2, A, B);
    core.add2(None, u32::MAX, B, A);
    core.present(MAIN, A, A);
    core.present(MAIN, A, B);
    core.present(MAIN, B, A);
    core.assert_uncounted();

    core.remove(A, A);
    core.remove(A, B);
    core.remove(B, A);
    core.absent(MAIN, A, A);
    core.absent(MAIN, A, B);
    core.absent(MAIN, B, A);
    assert!(!core.active(MAIN));
}

#[test]
fn test_raw_video_callbacks_zero_divisor_and_null_callback_register_nothing() {
    let mut core = Core::setup();
    core.add2(None, 0, A, A);
    core.absent(MAIN, A, A);
    core.add(None, None, A);
    core.add2(None, 1, None, None);
    core.absent(MAIN, None, A);
    assert!(!core.active(MAIN));
    // Removing a NULL callback is harmless too.
    core.remove(None, None);
    assert!(!core.active(MAIN));
    core.assert_uncounted();
}

#[test]
fn test_raw_video_callbacks_repeated_add_is_one_registration() {
    let mut core = Core::setup();
    core.add(None, A, A);
    core.add(None, A, A);
    // A later add2 with another divisor is still the same pair.
    core.add2(None, 3, A, A);
    core.present(MAIN, A, A);

    // One removal takes it out completely: the repeats kept no copy.
    core.remove(A, A);
    core.absent(MAIN, A, A);
    assert!(!core.active(MAIN));
    core.assert_uncounted();
}

#[test]
fn test_raw_video_callbacks_callback_and_parameter_identity() {
    let mut core = Core::setup();
    core.add(None, A, A);
    core.add(None, A, B);
    core.add(None, B, A);
    core.add(None, B, None);
    core.present(MAIN, A, A);
    core.present(MAIN, A, B);
    core.present(MAIN, B, A);
    core.present(MAIN, B, None);
    core.absent(MAIN, B, B);
    core.absent(MAIN, A, None);

    // A removal takes out only its own pair.
    core.remove(A, B);
    core.absent(MAIN, A, B);
    core.present(MAIN, A, A);
    core.present(MAIN, B, A);
    core.present(MAIN, B, None);

    // Removing a pair that was never added changes nothing.
    core.remove(B, B);
    core.remove(A, None);
    core.present(MAIN, A, A);
    core.present(MAIN, B, A);
    core.present(MAIN, B, None);

    core.remove(A, A);
    core.remove(B, A);
    assert!(core.active(MAIN));
    core.remove(B, None);
    assert!(!core.active(MAIN));
    core.assert_uncounted();
}

#[test]
fn test_raw_video_callbacks_conversion_is_accepted() {
    let mut core = Core::setup();
    // The conversion is passed through as is; it is not part of the pair.
    let conversion = Some(420);
    core.add(conversion, B, B);
    core.present(MAIN, B, B);
    core.add2(conversion, 2, A, B);
    core.present(MAIN, A, B);
    core.remove(B, B);
    core.remove(A, B);
    core.absent(MAIN, B, B);
    core.absent(MAIN, A, B);
    assert!(!core.active(MAIN));
}

#[test]
fn test_raw_video_callbacks_follows_the_current_main_mix() {
    let mut core = Core::setup();
    let stand_in = core.mix;
    core.add(None, A, A);

    // After the main mix changes, adds and removals go to the new video.
    core.mix = Some(Mix {
        video: OTHER,
        raw_active: 0,
    });
    core.add(None, B, B);
    core.present(OTHER, B, B);
    core.absent(MAIN, B, B);
    // The removal looks only at the current video.
    core.remove(A, A);
    core.present(MAIN, A, A);
    core.remove(B, B);
    core.absent(OTHER, B, B);
    core.assert_uncounted();

    core.mix = stand_in;
    core.remove(A, A);
    core.absent(MAIN, A, A);
    assert!(!core.active(MAIN));
    assert!(!core.active(OTHER));
    core.assert_uncounted();
}

/// test_main_mix_before_video's raw callback calls: with no mix the add
/// warns and keeps nothing, the removal does nothing, and a mix set up
/// afterwards starts empty. With no main canvas at all, the same.
#[test]
fn test_raw_video_callbacks_before_first_video_reset() {
    for canvas in [false, true] {
        let mut core = Core::setup();
        core.canvas = canvas;
        let stand_in = core.mix.take();
        core.add(None, A, A);
        core.add2(None, 2, B, B);
        core.remove(A, A);
        assert_eq!(core.calls, [Call::Warn, Call::Warn]);
        core.canvas = true;
        core.mix = stand_in;
        core.absent(MAIN, A, A);
        core.absent(MAIN, B, B);
        assert!(!core.active(MAIN));
        core.assert_uncounted();
    }
}

//! Safe mirrors of test_get_video.c, plus the NULL main canvas the C test
//! cannot reach (the core global is hidden from test executables).
use obs_core::get_video::get_video;
use std::cell::Cell;

#[derive(Debug, PartialEq, Eq)]
struct Video(u8);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mix<'a> {
    video: Option<&'a Video>,
}

struct Canvas<'a> {
    mix: Cell<Option<&'a Mix<'a>>>,
    refs: Cell<i64>,
    weak_refs: Cell<i64>,
}

impl<'a> Canvas<'a> {
    fn new() -> Self {
        Canvas {
            mix: Cell::new(None),
            refs: Cell::new(1),
            weak_refs: Cell::new(1),
        }
    }

    fn refs(&self) -> (i64, i64) {
        (self.refs.get(), self.weak_refs.get())
    }
}

/// `obs_get_video()` over the safe core, reading the main canvas's fields
/// the way the C adapters do: borrowed, with no reference taken.
fn obs_get_video<'a>(main: Option<&'a Canvas<'a>>) -> Option<&'a Video> {
    get_video(main, |canvas| canvas.mix.get(), |mix| mix.video)
}

static VIDEO_A: Video = Video(1);
static VIDEO_B: Video = Video(2);

#[test]
fn main_canvas_without_mix_returns_null() {
    let canvas = Canvas::new();
    let before = canvas.refs();
    for _ in 0..16 {
        assert!(obs_get_video(Some(&canvas)).is_none());
    }
    assert!(canvas.mix.get().is_none());
    assert_eq!(canvas.refs(), before);
}

#[test]
fn main_canvas_mix_video_is_borrowed_by_identity() {
    let canvas = Canvas::new();
    assert!(canvas.mix.get().is_none());
    let mix = Mix {
        video: Some(&VIDEO_A),
    };
    canvas.mix.set(Some(&mix));
    let refs = canvas.refs();
    let before = mix.clone();
    for _ in 0..16 {
        assert!(core::ptr::eq(
            obs_get_video(Some(&canvas)).unwrap(),
            &VIDEO_A
        ));
    }
    assert_eq!(mix, before);
    assert!(core::ptr::eq(canvas.mix.get().unwrap(), &mix));
    assert_eq!(canvas.refs(), refs);

    // The getter follows the current mix's video, not a cached value.
    let mix_b = Mix {
        video: Some(&VIDEO_B),
    };
    canvas.mix.set(Some(&mix_b));
    assert!(core::ptr::eq(
        obs_get_video(Some(&canvas)).unwrap(),
        &VIDEO_B
    ));
    assert_eq!(mix_b.video, Some(&VIDEO_B));

    // A mix whose video is not set yet answers NULL, not the stale one.
    let mix_none = Mix { video: None };
    canvas.mix.set(Some(&mix_none));
    assert!(obs_get_video(Some(&canvas)).is_none());
    assert!(core::ptr::eq(canvas.mix.get().unwrap(), &mix_none));
    assert_eq!(canvas.refs(), refs);

    canvas.mix.set(None);
    assert!(obs_get_video(Some(&canvas)).is_none());
}

#[test]
fn auxiliary_canvas_mix_is_not_the_main_video() {
    let main = Canvas::new();
    let other = Canvas::new();
    let mix = Mix {
        video: Some(&VIDEO_B),
    };
    other.mix.set(Some(&mix));
    assert!(main.mix.get().is_none());
    assert!(obs_get_video(Some(&main)).is_none());
}

#[test]
fn null_main_canvas_returns_null_without_reading_further() {
    let video = get_video(
        None::<&Canvas>,
        |_| -> Option<&Mix> { panic!("no canvas to read a mix from") },
        |_| -> Option<&Video> { panic!("no mix to read a video from") },
    );
    assert!(video.is_none());
}

#[test]
fn missing_mix_never_reads_a_video() {
    let canvas = Canvas::new();
    let video = get_video(
        Some(&canvas),
        |canvas| canvas.mix.get(),
        |_| -> Option<&Video> { panic!("no mix to read a video from") },
    );
    assert!(video.is_none());
}

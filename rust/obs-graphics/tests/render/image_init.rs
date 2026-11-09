use Operation::{
    Allocate, BestStream, BufferTooLarge, CloseFormat, Configure, DefaultBuffer, FindDecoder,
    FirstKnown, FirstVideo, FreeCodec, Open, OpenCodec, Probe, StreamCount, TooLarge, WarnOpen,
};
use obs_graphics::image_init::{self, Backend, Operation};

#[derive(Default)]
struct Fake {
    calls: Vec<(Operation, i32)>,
    fail: Option<Operation>,
    probe: bool,
    oversized: bool,
}
impl Backend for Fake {
    fn run(&mut self, op: Operation, value: i32) -> i32 {
        self.calls.push((op, value));
        if self.fail == Some(op) {
            return -7;
        }
        match op {
            StreamCount => {
                if self.probe {
                    2
                } else {
                    1
                }
            }
            FirstVideo | FirstKnown | FindDecoder => 1,
            TooLarge => i32::from(self.oversized),
            BufferTooLarge if self.oversized => -22,
            _ => 0,
        }
    }
}

#[test]
fn test_ffmpeg_image_init_empty_path_leaves_state_untouched() {
    let mut b = Fake::default();
    assert!(!image_init::init(&mut b, false));
    assert!(b.calls.is_empty());
}
#[test]
fn test_ffmpeg_image_init_missing_and_corrupt_files() {
    for op in [Open, Probe, BestStream, OpenCodec] {
        let mut b = Fake {
            fail: Some(op),
            probe: true,
            ..Fake::default()
        };
        assert!(!image_init::init(&mut b, true));
        if op == Open {
            assert_eq!(b.calls.last(), Some(&(WarnOpen, -7)));
        } else {
            assert!(b.calls.ends_with(&[(FreeCodec, 0), (CloseFormat, 0)]));
        }
    }
}
#[test]
fn test_ffmpeg_image_init_valid_png_skips_probe_and_repeated_cleanup() {
    for _ in 0..8 {
        let mut b = Fake::default();
        assert!(image_init::init(&mut b, true));
        assert!(!b.calls.iter().any(|(op, _)| *op == Probe));
        assert!(b.calls.iter().any(|(op, _)| *op == Configure));
        image_init::free(&mut b);
        assert!(b.calls.ends_with(&[(FreeCodec, 0), (CloseFormat, 0)]));
    }
}
#[test]
fn oversized_image_never_allocates_decoder_or_frame() {
    let mut b = Fake {
        oversized: true,
        ..Fake::default()
    };
    assert!(!image_init::init(&mut b, true));
    assert!(!b.calls.iter().any(|(op, _)| *op == Allocate));
    assert!(image_init::get_buffer(&mut b, 3) < 0);
    assert!(!b.calls.iter().any(|(op, _)| *op == DefaultBuffer));
}
#[test]
fn buffer_flags_and_ffmpeg_error_propagate() {
    let mut b = Fake {
        fail: Some(DefaultBuffer),
        ..Fake::default()
    };
    assert_eq!(image_init::get_buffer(&mut b, 42), -7);
    assert_eq!(b.calls.last(), Some(&(DefaultBuffer, 42)));
}

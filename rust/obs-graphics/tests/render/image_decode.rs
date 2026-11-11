use obs_graphics::image_decode::{self, Decoder, Status, Warning};

#[derive(Default)]
pub(super) struct Fake {
    pub allocate: bool,
    pub read_error: Option<i32>,
    pub statuses: std::collections::VecDeque<Status>,
    pub too_large: bool,
    pub rejected: bool,
    pub events: Vec<i32>,
    pub pixels: Vec<u8>,
    pub image: (i32, i32, i32),
    pub frame: (i32, i32, i32),
}

impl Decoder for Fake {
    type Output = Vec<u8>;
    fn allocate(&mut self) -> bool {
        self.events.push(1);
        self.allocate
    }
    fn read(&mut self) -> Result<(), i32> {
        self.events.push(2);
        self.read_error.map_or(Ok(()), Err)
    }
    fn send(&mut self) -> Status {
        self.events.push(3);
        self.statuses.pop_front().unwrap_or(Status::Ready)
    }
    fn receive(&mut self) -> Status {
        self.events.push(4);
        self.statuses.pop_front().unwrap_or(Status::Ready)
    }
    fn too_large(&mut self) -> bool {
        self.events.push(5);
        self.too_large
    }
    fn check_size(&mut self) -> bool {
        self.events.push(6);
        self.rejected
    }
    fn update(&mut self) {
        self.events.push(7);
        self.image = self.frame;
    }
    fn reformat(&mut self) -> Vec<u8> {
        self.events.push(8);
        self.pixels.clone()
    }
    fn warn(&mut self, warning: Warning) {
        self.events.push(match warning {
            Warning::Allocate => 10,
            Warning::Read(_) => 11,
            Warning::Decode(_) => 12,
        });
    }
    fn cleanup(&mut self) {
        self.events.extend([13, 14]);
    }
}

// Mirrors test_ffmpeg_image_decode's PNG and JPEG output assertions at the
// orchestration boundary; unchanged Tier 2 exercises the real codecs/pixels.
#[test]
fn test_ffmpeg_image_decode_png_and_jpeg() {
    for (pixel, format) in [([32, 64, 96, 255], 26), ([64, 64, 64, 255], 28)] {
        let pixels = pixel.repeat(16 * 8);
        let mut d = Fake {
            allocate: true,
            pixels: pixels.clone(),
            image: (0, 0, 0),
            frame: (16, 8, format),
            ..Fake::default()
        };
        for _ in 0..2 {
            d.events.clear();
            assert_eq!(image_decode::decode(&mut d), Some(pixels.clone()));
            assert_eq!(d.image, (16, 8, format));
            assert_eq!(d.events, [1, 2, 3, 4, 6, 7, 8, 13, 14]);
        }
    }
}

#[test]
fn test_ffmpeg_image_decode_truncated_preserves_output() {
    let mut d = Fake {
        allocate: true,
        statuses: [Status::Error(-22)].into(),
        image: (123, 456, 0),
        ..Fake::default()
    };
    assert_eq!(image_decode::decode(&mut d), None);
    assert_eq!(d.events, [1, 2, 3, 5, 12, 13, 14]);
    assert_eq!(d.image, (123, 456, 0));
}

#[test]
fn failures_and_retries_preserve_cleanup_order() {
    let mut d = Fake::default();
    assert_eq!(image_decode::decode(&mut d), None);
    assert_eq!(d.events, [1, 10]);
    let mut d = Fake {
        allocate: true,
        read_error: Some(-5),
        ..Fake::default()
    };
    assert_eq!(image_decode::decode(&mut d), None);
    assert_eq!(d.events, [1, 2, 11, 13, 14]);
    let mut d = Fake {
        allocate: true,
        rejected: true,
        ..Fake::default()
    };
    assert_eq!(image_decode::decode(&mut d), None);
    assert_eq!(d.events, [1, 2, 3, 4, 6, 13, 14]);
    let mut d = Fake {
        allocate: true,
        statuses: [
            Status::Retry,
            Status::Ready,
            Status::Retry,
            Status::Ready,
            Status::Ready,
        ]
        .into(),
        ..Fake::default()
    };
    assert_eq!(image_decode::decode(&mut d), Some(vec![]));
    assert_eq!(d.events, [1, 2, 3, 3, 4, 3, 4, 6, 7, 8, 13, 14]);
}

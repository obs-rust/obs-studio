//! Tier 1 mirrors of test_source_frame_init.c, through the safe API.
use obs_media_io::video_frame::FrameLayout;
use obs_media_io::video_io::VideoFormat;
use obs_source::frame_init::{Destination, Initialized, Planes, init};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Frame {
    data: [Option<usize>; 8],
    linesize: [u32; 8],
    width: u32,
    height: u32,
    format: i32,
    // Timestamp, colour metadata, flags, refs and padding are not initialized.
    unrelated: [u8; 160],
}

impl Destination for Frame {
    type Plane = Option<usize>;
    fn store(&mut self, value: Initialized<Self::Plane>) {
        self.format = value.format;
        self.width = value.width;
        self.height = value.height;
        self.data = value.planes.data;
        self.linesize = value.planes.linesize;
    }
}

fn sentinel() -> Frame {
    Frame {
        data: [Some(0x5a5a); 8],
        linesize: [0x5a5a5a5a; 8],
        width: 0x5a5a5a5a,
        height: 0x5a5a5a5a,
        format: 0x5a5a5a5a,
        unrelated: [0x5a; 160],
    }
}

#[test]
fn test_source_frame_init_null_frame_logs_the_caller() {
    let mut logs = Vec::new();
    init::<Frame>(
        None,
        VideoFormat::Rgba as i32,
        13,
        7,
        |_, _, _| panic!("NULL must not allocate"),
        |level, message| logs.push((level, message)),
    );
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].0, 400);
    assert_eq!(logs[0].1, c"obs_source_frame_init: Null 'frame' parameter");
}

#[test]
fn test_source_frame_init_plane_layout_and_unrelated_bytes() {
    for format in [
        VideoFormat::Rgba,
        VideoFormat::I420,
        VideoFormat::Nv12,
        VideoFormat::I444,
        VideoFormat::Yuva,
        VideoFormat::P010,
    ] {
        for (width, height) in [(1, 1), (13, 7), (32, 16)] {
            let layout = FrameLayout::new(format, width, height);
            let mut frame = sentinel();
            let mut expected = frame.clone();
            init(
                Some(&mut frame),
                format as i32,
                width,
                height,
                |f, w, h| {
                    assert_eq!((f, w, h), (format as i32, width, height));
                    Planes {
                        data: layout.offset.map(|offset| offset.map(|n| 4096 + n)),
                        linesize: layout.linesize,
                    }
                },
                |_, _| panic!("valid frame logged"),
            );
            expected.format = format as i32;
            expected.width = width;
            expected.height = height;
            for i in 0..8 {
                assert_eq!(frame.linesize[i], layout.linesize[i]);
                match layout.offset[i] {
                    Some(offset) => {
                        assert!(frame.data[i].is_some());
                        assert_eq!(frame.data[i].unwrap() - frame.data[0].unwrap(), offset);
                    }
                    None => assert_eq!(frame.data[i], None),
                }
                expected.data[i] = frame.data[i];
                expected.linesize[i] = layout.linesize[i];
            }
            assert_eq!(frame, expected);
        }
    }
}

#[test]
fn all_eight_slots_are_transferred_even_when_the_last_plane_is_present() {
    let mut frame = sentinel();
    let data = core::array::from_fn(|i| Some(1000 + i));
    let linesize = core::array::from_fn(|i| i as u32 + 1);
    init(
        Some(&mut frame),
        -7,
        19,
        23,
        |_, _, _| Planes { data, linesize },
        |_, _| panic!("valid frame logged"),
    );
    assert_eq!(frame.data, data);
    assert_eq!(frame.linesize, linesize);
    assert_eq!((frame.format, frame.width, frame.height), (-7, 19, 23));
    assert_eq!(frame.unrelated, [0x5a; 160]);
}

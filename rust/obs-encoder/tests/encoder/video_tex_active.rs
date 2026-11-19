//! Tier 1 mirrors test_encoder_video_tex_active.c without private C layouts.
use obs_encoder::video_tex_active::{NV12, P010, video_tex_active};
use std::cell::{Cell, RefCell};

fn check(format: i32, media: Option<usize>, mixed: bool, flags: [bool; 2]) {
    let encoder = (media, 0x5a_u8);
    let before = encoder;
    let mix = Cell::new([!flags[0], !flags[1]]);
    let events = RefCell::new(Vec::new());
    let result = video_tex_active(
        format,
        || {
            assert_eq!(encoder.0, media);
            events.borrow_mut().push("lookup");
            // Like the characterization, make pre-lookup reads observably wrong.
            mix.set(flags);
            mixed.then_some(&mix)
        },
        |selected| {
            assert!(std::ptr::eq(selected, &mix));
            events.borrow_mut().push("nv12");
            selected.get()[0]
        },
        |selected| {
            assert!(std::ptr::eq(selected, &mix));
            events.borrow_mut().push("p010");
            selected.get()[1]
        },
    );
    assert_eq!(
        result,
        mixed && ((format == NV12 && flags[0]) || (format == P010 && flags[1]))
    );
    let mut expected = vec!["lookup"];
    if mixed && format == NV12 {
        expected.push("nv12");
    } else if mixed && format == P010 {
        expected.push("p010");
    }
    assert_eq!(*events.borrow(), expected);
    assert_eq!(encoder, before);
    assert_eq!(mix.get(), flags);
}

#[test]
fn test_encoder_video_tex_active_missing_mix() {
    for media in [None, Some(11)] {
        for format in [NV12, P010, 6, 0] {
            check(format, media, false, [true, true]);
        }
    }
}

#[test]
fn test_encoder_video_tex_active_format_and_flag_matrix() {
    for flags in [[false, false], [true, false], [false, true], [true, true]] {
        for format in 0..=25 {
            check(format, Some(11), true, flags);
        }
    }
}

#[test]
fn test_encoder_video_tex_active_reads_selected_mix_after_lookup() {
    for flags in [[false, false], [true, false], [false, true], [true, true]] {
        for format in [NV12, P010] {
            check(format, Some(11), true, flags);
        }
    }
}

#[test]
fn test_encoder_tex_active_null_no_video_and_canvas_gone() {
    // A live encoder before/after video reset, or after its canvas mix is gone.
    for media in [None, Some(11)] {
        for format in [NV12, P010, 1] {
            check(format, media, false, [false, false]);
        }
    }
}

#[test]
fn test_encoder_tex_active_null_main_video_capabilities() {
    // Model capabilities; make no assertion about any real device's support.
    for available in [false, true] {
        for (flags, format) in [([available, false], NV12), ([false, available], P010)] {
            for requested in [NV12, P010, 1] {
                check(requested, Some(11), true, flags);
            }
            check(format, Some(11), true, flags);
        }
    }
}

#[test]
fn other_formats_still_look_up_without_reading_flags() {
    for format in [i32::MIN, -1, 26, i32::MAX] {
        check(format, None, true, [true, true]);
    }
}

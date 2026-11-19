//! Safe mirrors of test/cmocka/test_video_info_query.c.
use obs_core::video_info_query::video_info;

#[test]
fn test_video_info_query_no_core() {
    assert_eq!(video_info::<(), u8>(None, |_| panic!("no core"), |_| panic!("no core")), None);
}

#[test]
fn test_video_info_query_no_graphics() {
    for canvas in [None, Some(())] {
        assert_eq!(video_info(Some(canvas), |_| false, |_| -> Option<u8> { panic!("no graphics") }), None);
    }
}

#[test]
fn test_video_info_query_no_mix() {
    assert_eq!(video_info(Some(()), |_| true, |_| None::<u8>), None);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Info<'a> {
    graphics_module: Option<&'a str>,
    fps_num: u32,
    fps_den: u32,
    base_width: u32,
    base_height: u32,
    output_width: u32,
    output_height: u32,
    output_format: i32,
    adapter: u32,
    gpu_conversion: bool,
    colorspace: i32,
    range: i32,
    scale_type: i32,
}

#[test]
fn test_video_info_query_complete_copy() {
    for alternate in [false, true] {
        let stored = Info {
            graphics_module: (!alternate).then_some("borrowed graphics module"),
            fps_num: if alternate { 0 } else { u32::MAX },
            fps_den: if alternate { u32::MAX } else { 0 },
            base_width: u32::from(alternate), base_height: 7919,
            output_width: u32::MAX, output_height: 101,
            output_format: if alternate { 6 } else { 2 }, adapter: 0x12345678,
            gpu_conversion: alternate, colorspace: if alternate { 2 } else { 3 },
            range: if alternate { 2 } else { 1 }, scale_type: if alternate { 1 } else { 4 },
        };
        let before = stored;
        let output = video_info(Some(&stored), |_| true, Some).unwrap();
        assert_eq!(output, &before);
        assert!(core::ptr::eq(output, &stored));
        assert_eq!(stored, before);
    }
}

#[test]
fn unavailable_preserves_every_output_byte() {
    for byte in 0..=u8::MAX {
        let mut output = [byte; 64];
        for (core, graphics, mixed) in [(false, true, true), (true, false, true), (true, true, false)] {
            if let Some(info) = video_info(core.then_some(()), |_| graphics, |_| mixed.then_some([0; 64])) {
                output = info;
            }
            assert_eq!(output, [byte; 64]);
        }
    }
}

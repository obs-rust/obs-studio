//! Tier 1: mirrors test_output_size_getters.c without private layouts or FFI.
use core::cell::RefCell;
use core::ffi::CStr;
use obs_output::output_size_getters::{Access, Axis, size};
use obs_output::video_checks::{OBS_OUTPUT_ENCODED, OBS_OUTPUT_VIDEO, SizeRoute};

struct Output {
    flags: u32,
    scaled: [u32; 2],
    video: [u32; 2],
    encoder: Option<(usize, [u32; 2])>,
    logs: RefCell<Vec<(&'static CStr, bool)>>,
}
impl Access for Output {
    fn flags(&self) -> u32 {
        self.flags
    }
    fn scaled(&self, axis: Axis) -> u32 {
        self.scaled[usize::from(axis == Axis::Height)]
    }
    fn has_encoder(&self, idx: usize) -> bool {
        self.encoder.is_some_and(|(slot, _)| slot == idx)
    }
    fn read(&self, idx: usize, axis: Axis, source: SizeRoute) -> u32 {
        let axis = usize::from(axis == Axis::Height);
        match source {
            SizeRoute::Encoder => {
                let (slot, dimensions) = self.encoder.unwrap();
                assert_eq!(idx, slot);
                dimensions[axis]
            }
            SizeRoute::Video => self.video[axis],
            _ => panic!("zero/scaled routes must not request external dimensions"),
        }
    }
    fn log(&self, name: &'static CStr, null: bool) {
        self.logs.borrow_mut().push((name, null));
    }
}
// The expected values from the C characterization, independent of the core.
fn expected(encoded: bool, present: u32, dims: [u32; 2], output: &Output, axis: Axis) -> u32 {
    let a = usize::from(axis == Axis::Height);
    if encoded {
        if present == 0 { 0 } else { dims[a] }
    } else if output.scaled[a] != 0 {
        output.scaled[a]
    } else {
        output.video[a]
    }
}
#[test]
fn test_output_size_getters_characterize() {
    for axis in [Axis::Width, Axis::Height] {
        for encoded in [false, true] {
            let mut o = Output {
                flags: 2 | if encoded { OBS_OUTPUT_ENCODED } else { 0 },
                scaled: [123, 456],
                video: [320, 240],
                encoder: None,
                logs: RefCell::new(vec![]),
            };
            for idx in [Some(usize::MAX), None] {
                assert_eq!(size(false, idx, axis, &o), 0);
                assert_eq!(o.logs.replace(vec![]), [(axis.name(idx.is_some()), true)]);
            }
            for idx in [Some(usize::MAX), Some(0), None] {
                assert_eq!(size(true, idx, axis, &o), 0);
                assert_eq!(o.logs.replace(vec![]), [(axis.name(idx.is_some()), false)]);
            }
            o.flags |= OBS_OUTPUT_VIDEO;
            for idx in [10, usize::MAX] {
                assert_eq!(size(true, Some(idx), axis, &o), 0);
            }
            for media in [false, true] {
                o.video = if media { [320, 240] } else { [0, 0] };
                for width in [0, 1, 1920, u32::MAX] {
                    for height in [0, 1, 1920, u32::MAX] {
                        o.scaled = [width, height];
                        for idx in 0..10 {
                            for present in 0..4 {
                                let dims = match present {
                                    2 => [320, 240],
                                    3 => [0, 0],
                                    _ => [640 + idx as u32, 480 + idx as u32],
                                };
                                o.encoder = (present != 0).then_some((idx, dims));
                                let want = expected(encoded, present, dims, &o, axis);
                                assert_eq!(size(true, Some(idx), axis, &o), want);
                                assert_eq!(
                                    size(true, None, axis, &o),
                                    if encoded && idx != 0 { 0 } else { want }
                                );
                            }
                        }
                    }
                }
            }
            assert!(o.logs.borrow().is_empty());
        }
    }
}
struct Reject;
impl Access for Reject {
    fn flags(&self) -> u32 {
        panic!("NULL read")
    }
    fn scaled(&self, _: Axis) -> u32 {
        panic!("invalid index read")
    }
    fn has_encoder(&self, _: usize) -> bool {
        panic!("invalid index read")
    }
    fn read(&self, _: usize, _: Axis, _: SizeRoute) -> u32 {
        panic!("invalid source read")
    }
    fn log(&self, _: &'static CStr, _: bool) {}
}
#[test]
fn null_is_rejected_before_any_adapter_read() {
    for axis in [Axis::Width, Axis::Height] {
        for idx in [None, Some(0), Some(usize::MAX)] {
            assert_eq!(size(false, idx, axis, &Reject), 0);
        }
    }
}

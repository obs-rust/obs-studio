//! Output dimensions with validation before indexing and lazy source reads.
use crate::video_checks::SizeRoute;
use core::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Width,
    Height,
}
impl Axis {
    pub fn name(self, indexed: bool) -> &'static CStr {
        match (self, indexed) {
            (Self::Width, true) => c"obs_output_get_width2",
            (Self::Width, false) => c"obs_output_get_width",
            (Self::Height, true) => c"obs_output_get_height2",
            (Self::Height, false) => c"obs_output_get_height",
        }
    }
}
/// Private output access is delayed until the corresponding validation succeeds.
pub trait Access {
    fn flags(&self) -> u32;
    fn scaled(&self, axis: Axis) -> u32;
    fn has_encoder(&self, idx: usize) -> bool;
    fn read(&self, idx: usize, axis: Axis, source: SizeRoute) -> u32;
    fn log(&self, name: &'static CStr, null: bool);
}
/// `None` selects the default-index wrapper, preserving its diagnostic name.
pub fn size(valid: bool, idx: Option<usize>, axis: Axis, output: &impl Access) -> u32 {
    use crate::output_slots::{SlotKind, slot_exists};
    use crate::video_checks::{OBS_OUTPUT_ENCODED, OBS_OUTPUT_VIDEO, size_route};
    let name = axis.name(idx.is_some());
    if !valid {
        output.log(name, true);
        return 0;
    }
    let flags = output.flags();
    if flags & OBS_OUTPUT_VIDEO == 0 {
        output.log(name, false);
        return 0;
    }
    let idx = idx.unwrap_or(0);
    if !slot_exists(SlotKind::Video, idx) {
        return 0;
    }
    let scaled = output.scaled(axis);
    match size_route(
        flags & OBS_OUTPUT_ENCODED != 0,
        output.has_encoder(idx),
        scaled,
    ) {
        SizeRoute::Zero => 0,
        SizeRoute::Scaled => scaled,
        source => output.read(idx, axis, source),
    }
}

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
pub fn size(_valid: bool, _idx: Option<usize>, _axis: Axis, _output: &impl Access) -> u32 {
    todo!("#4594 output size getters")
}

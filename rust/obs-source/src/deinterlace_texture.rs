//! Allocation sequence for the previous deinterlace frame, without graphics state.
use crate::conversion_plan::{gs_color_format as gs, video_format as vf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Plane { pub width: u32, pub height: u32, pub format: i32 }

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub format: i32, pub trc: u8, pub gpu: bool, pub channels: i32,
    pub width: u32, pub height: u32, pub planes: [Plane; 4],
}

/// Each callback creates and stores its destination immediately, including failures.
pub trait Allocator {
    fn render(&mut self, format: i32, zs: i32);
    fn texture(&mut self, index: usize, plane: Plane, levels: u32, flags: u32);
}

pub fn allocate(input: Input, allocator: &mut impl Allocator) {
    let _ = (input, allocator);
    todo!()
}

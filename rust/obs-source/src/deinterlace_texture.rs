//! Allocation sequence for the previous deinterlace frame, without graphics state.
use crate::conversion_plan::{gs_color_format as gs, video_format as vf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Plane {
    pub width: u32,
    pub height: u32,
    pub format: i32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub format: i32,
    pub trc: u8,
    pub gpu: bool,
    pub channels: i32,
    pub width: u32,
    pub height: u32,
    pub planes: [Plane; 8],
}

/// Each callback creates and stores its destination immediately, including failures.
pub trait Allocator {
    fn render(&mut self, format: i32, zs: i32);
    fn texture(&mut self, index: usize, plane: Plane, levels: u32, flags: u32);
}

/// Counts above the fixed plane capacity are bounded by the safe API; they
/// index out of bounds in C and are excluded from parity. Negative counts
/// still create the GPU render target and allocate no planes.
pub fn allocate(input: Input, allocator: &mut impl Allocator) {
    let format = match (input.trc, input.format) {
        (2 | 3, _) => gs::RGBA16F,
        (_, vf::RGBA) => gs::RGBA,
        (_, vf::BGRA | vf::I40A | vf::I42A | vf::YUVA | vf::AYUV) => gs::BGRA,
        (
            _,
            vf::I010
            | vf::P010
            | vf::I210
            | vf::I412
            | vf::YA2L
            | vf::P216
            | vf::P416
            | vf::V210
            | vf::R10L,
        ) => gs::RGBA16F,
        _ => gs::BGRX,
    };
    if input.gpu {
        allocator.render(format, 0);
        for (index, plane) in input
            .planes
            .into_iter()
            .take(usize::try_from(input.channels).unwrap_or(0))
            .enumerate()
        {
            allocator.texture(index, plane, 1, 2);
        }
    } else {
        allocator.texture(
            0,
            Plane {
                width: input.width,
                height: input.height,
                format,
            },
            1,
            2,
        );
    }
}

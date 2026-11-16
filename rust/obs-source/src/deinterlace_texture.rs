//! Allocation sequence for the previous deinterlace frame, without graphics state.
use crate::conversion_plan::{gs_color_format as gs, video_format as vf};

/// The plane capacity of the source's arrays, `MAX_AV_PLANES`.
pub const PLANES: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Plane {
    pub width: u32,
    pub height: u32,
    pub format: i32,
}

/// The source fields the allocation reads. Each read is a call, made where
/// the C reads the field (#2153): the format and transfer first, then the
/// GPU flag, then on the GPU path the channel count before every plane and
/// each plane just before its texture (after the allocations before it), and
/// on the CPU path the async size.
pub trait Fields {
    fn format(&self) -> i32;
    fn trc(&self) -> u8;
    fn gpu(&self) -> bool;
    fn channels(&self) -> i32;
    /// The converted size and texture format of plane `index`, below
    /// [`PLANES`].
    fn plane(&self, index: usize) -> Plane;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
}

/// A source whose fields do not change, for callers that hold the values.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub format: i32,
    pub trc: u8,
    pub gpu: bool,
    pub channels: i32,
    pub width: u32,
    pub height: u32,
    pub planes: [Plane; PLANES],
}

impl Fields for Input {
    fn format(&self) -> i32 {
        self.format
    }
    fn trc(&self) -> u8 {
        self.trc
    }
    fn gpu(&self) -> bool {
        self.gpu
    }
    fn channels(&self) -> i32 {
        self.channels
    }
    fn plane(&self, index: usize) -> Plane {
        self.planes[index]
    }
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
}

/// Each callback creates and stores its destination immediately, including failures.
pub trait Allocator {
    fn render(&mut self, format: i32, zs: i32);
    fn texture(&mut self, index: usize, plane: Plane, levels: u32, flags: u32);
}

/// `convert_video_format()`: the texture format of a video format and
/// transfer.
fn convert_format(format: i32, trc: u8) -> i32 {
    match (trc, format) {
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
    }
}

/// Counts above the fixed plane capacity are bounded by the safe API; they
/// index out of bounds in C and are excluded from parity. Negative counts
/// still create the GPU render target and allocate no planes.
pub fn allocate(source: impl Fields, allocator: &mut impl Allocator) {
    let format = convert_format(source.format(), source.trc());
    if source.gpu() {
        allocator.render(format, 0);
        for index in 0..PLANES {
            // The C's `c < source->async_channel_count`, read again before
            // every plane.
            if i32::try_from(index).map_or(true, |c| c >= source.channels()) {
                break;
            }
            allocator.texture(index, source.plane(index), 1, 2);
        }
    } else {
        allocator.texture(
            0,
            Plane {
                width: source.width(),
                height: source.height(),
                format,
            },
            1,
            2,
        );
    }
}

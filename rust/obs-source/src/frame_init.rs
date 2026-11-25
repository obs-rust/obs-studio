//! Initialize only the geometry and eight planes of a source frame.
use core::ffi::CStr;

/// Allocated video planes; ownership passes to the source frame.
pub struct Planes<P> {
    pub data: [P; 8],
    pub linesize: [u32; 8],
}

/// The fields changed by `obs_source_frame_init`.
pub struct Initialized<P> {
    pub format: i32,
    pub width: u32,
    pub height: u32,
    pub planes: Planes<P>,
}

/// A frame destination that preserves all fields outside `Initialized`.
pub trait Destination {
    type Plane;
    fn store(&mut self, initialized: Initialized<Self::Plane>);
}

/// Allocate before updating the destination; NULL logs once without allocation.
pub fn init<D: Destination>(
    _frame: Option<&mut D>,
    _format: i32,
    _width: u32,
    _height: u32,
    _allocate: impl FnOnce(i32, u32, u32) -> Planes<D::Plane>,
    _log: impl FnOnce(i32, &'static CStr),
) {
    todo!("obs_source_frame_init Rust swap")
}

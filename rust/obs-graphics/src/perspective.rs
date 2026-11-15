//! `gs_perspective` (`libobs/graphics/graphics-perspective.c.h`): the
//! context check and the frustum it hands the device.
//!
//! The C computes, in `float` and in this order,
//! `ymax = near * tanf(RAD(angle) * 0.5f)`, `ymin = -ymax`,
//! `xmin = ymin * aspect` and `xmax = ymax * aspect`, then calls
//! `device_frustum(device, xmin, xmax, ymin, ymax, near, far)` once. No
//! argument is range checked, as in the C. `f32::tan` is the platform's
//! `tanf`, the function the C calls, so every argument matches bit for bit.

use std::ffi::CStr;

/// `RAD()`'s `float` constant (`libobs/graphics/math-defs.h`).
pub const RAD: f32 = 0.017_453_292_519_943_295_769_236_907_684_8;

/// The debug line `gs_valid()` logs, after the function name and `": "`.
pub const NOT_IN_CONTEXT: &str = "called while not in a graphics context";

/// The name `gs_valid()` logs.
pub const NAME: &CStr = c"gs_perspective";

/// The six `device_frustum` arguments after the device, in call order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frustum {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Frustum {
    /// The arguments as bit patterns, for exact comparison (signed zeros and
    /// NaN payloads included).
    #[must_use]
    pub fn bits(self) -> [u32; 6] {
        [
            self.left.to_bits(),
            self.right.to_bits(),
            self.top.to_bits(),
            self.bottom.to_bits(),
            self.znear.to_bits(),
            self.zfar.to_bits(),
        ]
    }
}

/// The context and device `gs_perspective` reaches through C.
pub trait Host {
    /// Whether this thread has a graphics context.
    fn has_context(&self) -> bool;
    /// `gs_valid()`'s `LOG_DEBUG` line for `function`.
    fn debug(&mut self, function: &'static CStr);
    /// `device_frustum` on the context's device.
    fn frustum(&mut self, frustum: Frustum);
}

/// The frustum `gs_perspective(angle, aspect, near, far)` passes the device.
#[must_use]
pub fn frustum(angle: f32, aspect: f32, near: f32, far: f32) -> Frustum {
    let _ = (angle, aspect, near, far);
    todo!("gs_perspective frustum")
}

/// `gs_perspective`: without a context, one debug line and no device call;
/// with one, exactly one `device_frustum`.
pub fn perspective(host: &mut impl Host, angle: f32, aspect: f32, near: f32, far: f32) {
    let _ = (host, angle, aspect, near, far);
    todo!("gs_perspective")
}

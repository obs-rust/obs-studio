//! Safe core of `libobs/obs-enter-leave-graphics.c.h`: `obs_enter_graphics()`
//! and `obs_leave_graphics()`, which enter and leave the core's graphics
//! context on the calling thread.
//!
//! Each reads the core's graphics subsystem (`obs->video.graphics`) once.
//! With none (before the first video reset) both do nothing, not even the
//! diagnostics the graphics layer would log. Otherwise enter forwards that
//! graphics to `gs_enter_context`, and leave calls `gs_leave_context()`, which
//! leaves whatever context the calling thread is in (and reports a leave from
//! outside any context). Nothing is written and no reference is taken; the
//! counting and the cross-thread wait are the graphics layer's.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// The core's graphics subsystem and the two graphics-layer calls.
pub trait Graphics {
    /// A non-NULL `graphics_t *`, borrowed for the enter call.
    type Context;
    /// One read of `obs->video.graphics`; `None` for NULL.
    fn graphics(&mut self) -> Option<Self::Context>;
    /// `gs_enter_context(graphics)`.
    fn enter_context(&mut self, graphics: Self::Context);
    /// `gs_leave_context()`.
    fn leave_context(&mut self);
}

/// `obs_enter_graphics()`: enter the core's graphics context when there is one.
pub fn enter_graphics<G: Graphics>(host: &mut G) {
    #[allow(clippy::todo)]
    {
        let _ = host;
        todo!()
    }
}

/// `obs_leave_graphics()`: leave the current graphics context when the core
/// has a graphics subsystem.
pub fn leave_graphics<G: Graphics>(host: &mut G) {
    #[allow(clippy::todo)]
    {
        let _ = host;
        todo!()
    }
}

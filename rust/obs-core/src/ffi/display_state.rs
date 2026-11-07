//! Display state ABI. Only the scalar prefix of the private `obs_display`
//! is projected; mutexes and the rest of the object stay opaque and C-owned.
//! Tier 3 measures every prefix field against the real `obs-internal.h`.
use crate::display_state;

/// Leading fields of `struct obs_display`, through `background_color`.
/// This is a view of a prefix, never an allocation for a complete display.
#[repr(C)]
pub struct DisplayStatePrefix {
    pub update_color_space: bool,
    pub enabled: bool,
    pub cx: u32,
    pub cy: u32,
    pub next_cx: u32,
    pub next_cy: u32,
    pub background_color: u32,
}

/// Set a display's enabled flag.
///
/// # Safety
/// A non-NULL pointer must name a live C display with exclusive access to
/// its enabled field for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_set_enabled(display: *mut DisplayStatePrefix, enable: bool) {
    // SAFETY: NULL becomes None; otherwise the caller guarantees a live field.
    let field = (!display.is_null()).then(|| unsafe { &mut (*display).enabled });
    display_state::set_enabled(field, enable);
}

/// Read a display's enabled flag.
///
/// # Safety
/// A non-NULL pointer must name a live C display with readable scalar fields
/// and no concurrent write for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_enabled(display: *mut DisplayStatePrefix) -> bool {
    // SAFETY: NULL becomes None; otherwise the caller guarantees a live prefix.
    let value = (!display.is_null()).then(|| unsafe { (*display).enabled });
    display_state::enabled(value)
}

/// Store the display's packed background color.
///
/// # Safety
/// A non-NULL pointer must name a live C display with exclusive access to
/// its background_color field for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_set_background_color(display: *mut DisplayStatePrefix, color: u32) {
    // SAFETY: NULL becomes None; otherwise the caller guarantees a live field.
    let field = (!display.is_null()).then(|| unsafe { &mut (*display).background_color });
    display_state::set_background_color(field, color);
}

//! Scalar display state accessors from `libobs/obs-display-state.c.h`.
//! Missing displays retain the C API's no-op setters and false getter.

/// Change only the enabled flag, if a display exists.
pub fn set_enabled(enabled: Option<&mut bool>, value: bool) {
    if let Some(enabled) = enabled {
        *enabled = value;
    }
}

/// Read the enabled flag; a missing display is disabled.
pub fn enabled(value: Option<bool>) -> bool {
    value.unwrap_or(false)
}

/// Store the packed color unchanged, including all alpha bits.
pub fn set_background_color(color: Option<&mut u32>, value: u32) {
    if let Some(color) = color {
        *color = value;
    }
}

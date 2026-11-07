//! Scalar display state accessors from `libobs/obs-display-state.c.h`.
//! Missing displays retain the C API's no-op setters and false getter.

/// Change only the enabled flag, if a display exists.
pub fn set_enabled(_enabled: Option<&mut bool>, _value: bool) {
    todo!("#3848 display enabled setter")
}

/// Read the enabled flag; a missing display is disabled.
pub fn enabled(_value: Option<bool>) -> bool {
    todo!("#3848 display enabled getter")
}

/// Store the packed color unchanged, including all alpha bits.
pub fn set_background_color(_color: Option<&mut u32>, _value: u32) {
    todo!("#3848 display background color setter")
}

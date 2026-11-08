//! Preferred video format selection without accessing the private encoder layout.

/// An absent video encoder has the C API's VIDEO_FORMAT_NONE default.
pub fn get(_format: Option<(i32, u32)>) -> u32 {
    todo!("port the preferred video format getter")
}

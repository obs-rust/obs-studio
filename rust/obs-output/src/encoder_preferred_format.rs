//! Preferred video format selection without accessing the private encoder layout.

/// Select the current video encoder format, or VIDEO_FORMAT_NONE for NULL/audio.
pub fn get(fields: Option<(i32, u32)>) -> u32 {
    match fields {
        Some((1, format)) => format,
        _ => 0,
    }
}

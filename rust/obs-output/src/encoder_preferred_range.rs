//! Preferred range selection without accessing the private encoder layout.

/// Select the current video encoder range, or VIDEO_RANGE_DEFAULT for NULL/audio.
pub fn get(fields: Option<(i32, u32)>) -> u32 {
    match fields {
        Some((1, range)) => range,
        _ => 0,
    }
}

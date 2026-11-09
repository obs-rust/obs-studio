//! Preferred color space selection without the private encoder layout.

/// Select the stored video color space, or VIDEO_CS_DEFAULT for NULL/audio.
pub fn get(fields: Option<(i32, u32)>) -> u32 {
    match fields {
        Some((1, space)) => space,
        _ => 0,
    }
}

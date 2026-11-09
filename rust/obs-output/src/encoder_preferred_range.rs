//! Stored video range selection independent of private encoder layout.

/// Return the stored video range or VIDEO_RANGE_DEFAULT for NULL/audio.
pub fn get(_fields: Option<(i32, u32)>) -> u32 {
    todo!("port obs_encoder_get_preferred_range")
}

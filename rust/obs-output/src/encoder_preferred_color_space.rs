//! Preferred color space selection without the private encoder layout.

/// Select the stored video color space, or VIDEO_CS_DEFAULT for NULL/audio.
pub fn get(_fields: Option<(i32, u32)>) -> u32 {
    todo!("port obs_encoder_get_preferred_color_space")
}

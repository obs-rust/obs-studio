//! Preferred video format mutation independent of the private C layout.

/// Preserve NULL and non-video no-ops; store video format bits unchanged.
pub fn set(fields: Option<(i32, &mut u32)>, format: u32) {
    let _ = (fields, format);
    todo!("port obs_encoder_set_preferred_video_format")
}

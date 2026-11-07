//! The current graphics context's linear-sRGB flag; no context means false.

pub fn linear_srgb(value: Option<bool>) -> bool {
    let _ = value;
    todo!("port gs_get_linear_srgb")
}

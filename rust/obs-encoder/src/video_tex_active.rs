//! Query C-owned texture-input state after the synchronized mix lookup.

pub const NV12: i32 = 2;
pub const P010: i32 = 18;

/// Look up exactly once, then read only the flag matching the requested format.
pub fn video_tex_active<M>(
    format: i32,
    lookup: impl FnOnce() -> Option<M>,
    nv12: impl FnOnce(M) -> bool,
    p010: impl FnOnce(M) -> bool,
) -> bool {
    let _ = (format, lookup, nv12, p010);
    todo!("swap the characterized texture-active query")
}

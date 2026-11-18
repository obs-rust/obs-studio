//! Exact default-flag assignment; current flags and ownership stay with C.

pub fn set_default_flags(current: Option<&mut u32>, flags: u32, invalid: impl FnOnce()) {
    let _ = (current, flags, invalid);
    todo!("obs_source_set_default_flags")
}

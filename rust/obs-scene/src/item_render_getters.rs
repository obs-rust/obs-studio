//! Read stored scale filter and blending values, preserving null diagnostics.

/// Missing items log once and return the original zero-valued enum default.
pub fn get(value: Option<u32>, log_null: impl FnOnce()) -> u32 {
    let _ = (value, log_null);
    todo!("#3889")
}

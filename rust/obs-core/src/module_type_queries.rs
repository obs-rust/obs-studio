//! Read-only classification of an optional module type.

/// Whether a live module is a core module; a missing handle is false.
pub fn is_core(module_type: Option<u32>) -> bool {
    let _ = module_type;
    todo!()
}

/// Whether a live module is a legacy plugin; a missing handle is false.
pub fn is_legacy(module_type: Option<u32>) -> bool {
    let _ = module_type;
    todo!()
}

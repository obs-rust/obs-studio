//! Read-only classification of an optional module type.

/// Whether a live module is a core module; a missing handle is false.
pub fn is_core(module_type: Option<u32>) -> bool {
    module_type.is_some_and(crate::module_load_rules::is_core)
}

/// Whether a live module is a legacy plugin; a missing handle is false.
pub fn is_legacy(module_type: Option<u32>) -> bool {
    module_type.is_some_and(crate::module_load_rules::is_legacy)
}

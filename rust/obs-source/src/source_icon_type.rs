//! Icon metadata lookup result, preserving the registered numeric value.

pub fn icon_type(registered: Option<u32>) -> u32 {
    registered.unwrap_or(0)
}

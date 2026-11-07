//! Module locale callback dispatch, without owning or interpreting translations.

/// Invoke the module callback when present, preserving its success result.
pub fn get_string(callback: Option<impl FnOnce() -> bool>) -> bool {
    let _ = callback;
    todo!("module locale callback dispatch")
}

/// Initialize the result with the input and retain every callback write.
pub fn get_text<T>(text: T, callback: Option<impl FnOnce(&mut T) -> bool>) -> T {
    let _ = (text, callback);
    todo!("module locale text fallback")
}

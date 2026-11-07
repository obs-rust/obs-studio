//! Module locale callback dispatch, without owning or interpreting translations.

/// Invoke the module callback when present, preserving its success result.
pub fn get_string(callback: Option<impl FnOnce() -> bool>) -> bool {
    callback.is_some_and(|callback| callback())
}

/// Initialize the result with the input and retain every callback write.
pub fn get_text<T>(text: T, callback: Option<impl FnOnce(&mut T) -> bool>) -> T {
    let mut result = text;
    get_string(callback.map(|callback| {
        let output = &mut result;
        move || callback(output)
    }));
    result
}

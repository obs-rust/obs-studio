//! Output connect-time callback query, independent of the private C layout.

/// Preserve the NULL diagnostic and missing-callback default.
pub fn get<T>(
    output: Option<T>,
    callback: impl FnOnce(T) -> Option<i32>,
    log: impl FnOnce(),
) -> i32 {
    match output {
        Some(output) => callback(output).unwrap_or(-1),
        None => {
            log();
            -1
        }
    }
}

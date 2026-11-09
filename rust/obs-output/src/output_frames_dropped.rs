//! Dropped-frame callback decision independent of the private C layout.
use crate::NullLog;

/// Preserve NULL logging, missing callback zero, and the exact signed result.
pub fn get<T>(output: Option<T>, query: impl FnOnce(T) -> Option<i32>, log: &impl NullLog) -> i32 {
    let _ = (output, query, log);
    todo!("port dropped-frame query")
}

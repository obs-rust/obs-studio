//! Dropped-frame callback decision independent of the private C layout.
use crate::NullLog;

/// Preserve NULL logging, missing callback zero, and the exact signed result.
pub fn get<T>(output: Option<T>, query: impl FnOnce(T) -> Option<i32>, log: &impl NullLog) -> i32 {
    match output {
        Some(output) => query(output).unwrap_or(0),
        None => {
            log.null_output(c"obs_output_get_frames_dropped");
            0
        }
    }
}

//! Output congestion query, independent of the private C layout.

/// Validate the output before querying the optional plugin callback.
pub fn get<T>(output: Option<T>, callback: impl FnOnce(T) -> Option<f32>, log: impl FnOnce()) -> f32 {
    todo!()
}

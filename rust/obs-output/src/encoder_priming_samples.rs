//! Encoder priming-sample callback dispatch, independent of private C layouts.

/// Return the installed callback's result, or zero when no callback is installed.
pub fn get_priming_samples<F: FnOnce() -> u32>(_callback: Option<F>) -> u32 {
    todo!("RED: encoder priming-sample dispatch")
}

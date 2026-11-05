//! Direct-output UYVX row kernels and their runtime selection (#3004).

/// Run parity against each row kernel without changing other test threads.
#[cfg(test)]
pub(crate) fn with_native_for_test(_native: bool, _run: impl FnOnce()) {
    todo!("forced direct-output UYVX row kernel")
}

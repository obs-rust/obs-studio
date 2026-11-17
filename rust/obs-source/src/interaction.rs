//! Capability-gated source interaction dispatch, independent of source layout.
use core::ffi::CStr;

pub const INTERACTION: u32 = 1 << 5;

/// Dispatch a borrowed payload unchanged, or diagnose a missing source.
/// Missing callbacks and sources without the interaction flag are no-ops.
pub fn dispatch<P, F: FnOnce(P)>(
    source: Option<(u32, Option<F>)>,
    payload: P,
    name: &'static CStr,
    log: impl FnOnce(&'static CStr),
) {
    let Some((flags, callback)) = source else {
        log(name);
        return;
    };
    if flags & INTERACTION != 0
        && let Some(callback) = callback
    {
        callback(payload);
    }
}

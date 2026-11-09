//! Capability-gated source interaction dispatch, independent of source layout.
use core::ffi::CStr;

pub const INTERACTION: u32 = 1 << 5;

/// Dispatch a borrowed payload unchanged, or diagnose a missing source.
/// Missing callbacks and sources without the interaction flag are no-ops.
pub fn dispatch<P, F: FnOnce(P)>(
    _source: Option<(u32, Option<F>)>,
    _payload: P,
    _name: &'static CStr,
    _log: impl FnOnce(&'static CStr),
) {
    todo!()
}

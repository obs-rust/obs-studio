//! Stored push delays and synchronous notification under the source audio mutex.
use crate::push_queries::PushFields;
use core::ffi::CStr;

/// Borrowed state. Notification may reenter on independent sources; no field
/// reference may span dispatch. Same-source locking reentry deadlocks as in C.
pub trait DelayFields: PushFields {
    fn store_delay(&mut self, talk: bool, delay: u64);
    fn signal_delay(&mut self, signal: &'static CStr, delay: i64);
}

pub fn set_delay<S: DelayFields>(
    source: Option<&mut S>,
    talk: bool,
    delay: u64,
    log: impl FnOnce(&'static CStr, &'static CStr),
) {
    todo!()
}

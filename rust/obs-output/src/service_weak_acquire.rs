//! Acquiring a strong service reference from its weak control (#3837).
//! Counts are biased by one: zero is one owner and -1 is expired.

use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

/// Increment the strong count only while the service is alive.
pub fn try_acquire(refs: &CLongAtomic) -> bool {
    refs.fetch_update(SeqCst, SeqCst, |owners| {
        (owners >= 0).then(|| owners.wrapping_add(1))
    })
    .is_ok()
}

/// The target is borrowed; taking the reference is a separate operation.
pub fn target<T: Copy>(service: T) -> T {
    service
}

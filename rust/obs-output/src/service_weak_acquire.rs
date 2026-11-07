//! Acquiring a strong service reference from its weak control (#3837).
//! Counts are biased by one: zero is one owner and -1 is expired.

use obs_util::platform::CLongAtomic;

/// Increment the strong count only while the service is alive.
pub fn try_acquire(_refs: &CLongAtomic) -> bool {
    todo!("#3837")
}

/// The target is borrowed; taking the reference is a separate operation.
pub fn target<T: Copy>(_service: T) -> T {
    todo!("#3837")
}

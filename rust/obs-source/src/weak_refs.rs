//! Source weak-handle reference counting. The weak count is biased by one:
//! 0 is the last owner, and the release that takes it to -1 frees the control.
use obs_util::platform::CLongAtomic;

/// Add one weak reference; `None` (a NULL handle) is a no-op.
pub fn addref(weak: Option<&CLongAtomic>) {
    let _ = weak;
    todo!("RED: obs_weak_source_addref")
}

/// Drop one weak reference; `true` when the caller must free the control.
/// `None` (a NULL handle) is a no-op.
pub fn release(weak: Option<&CLongAtomic>) -> bool {
    let _ = weak;
    todo!("RED: obs_weak_source_release")
}

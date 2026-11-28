//! Source weak-handle reference counting. The weak count is biased by one:
//! 0 is the last owner, and the release that takes it to -1 frees the control.
use crate::strong_refs::counted;
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

/// Add one weak reference; `None` (a NULL handle) is a no-op.
pub fn addref(weak: Option<&CLongAtomic>) {
    if let Some(weak) = weak {
        counted(weak, |weak| weak.fetch_add(1, SeqCst));
    }
}

/// Drop one weak reference; `true` when the caller must free the control.
/// `None` (a NULL handle) is a no-op. Only the exact step from 0 to -1 frees,
/// as in the C (`os_atomic_dec_long(...) == -1`).
pub fn release(weak: Option<&CLongAtomic>) -> bool {
    weak.is_some_and(|weak| counted(weak, |weak| weak.fetch_sub(1, SeqCst)) == 0)
}

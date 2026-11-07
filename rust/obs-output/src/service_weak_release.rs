//! Biased weak-control counts: zero represents one reference, -1 none.
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

/// Drop a strong reference; true only for the final owner.
pub fn strong_release(refs: &CLongAtomic) -> bool {
    refs.fetch_sub(1, SeqCst) == 0
}

/// Retain a weak reference, including the service's implicit weak owner.
pub fn weak_addref(refs: &CLongAtomic) {
    refs.fetch_add(1, SeqCst);
}

/// Drop a weak reference; true only when its allocation may be freed.
pub fn weak_release(refs: &CLongAtomic) -> bool {
    refs.fetch_sub(1, SeqCst) == 0
}

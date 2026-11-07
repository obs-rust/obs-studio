//! Biased weak-control counts: zero represents one reference, -1 none.
use obs_util::platform::CLongAtomic;

/// Drop a strong reference; true only for the final owner.
pub fn strong_release(_refs: &CLongAtomic) -> bool {
    todo!("#3838 strong release")
}

/// Retain a weak reference, including the service's implicit weak owner.
pub fn weak_addref(_refs: &CLongAtomic) {
    todo!("#3838 weak addref")
}

/// Drop a weak reference; true only when its allocation may be freed.
pub fn weak_release(_refs: &CLongAtomic) -> bool {
    todo!("#3838 weak release")
}

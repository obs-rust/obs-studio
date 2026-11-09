//! Canvas reference counts, biased by one: zero is one owner, -1 is expired.
use obs_util::platform::CLongAtomic;

/// Retain a weak reference; a missing handle is a no-op.
pub fn addref(_count: Option<&CLongAtomic>) {
    todo!()
}

/// Release a weak reference, returning whether its allocation must be freed.
pub fn release(_count: Option<&CLongAtomic>) -> bool {
    todo!()
}

/// Upgrade a live control and preserve its target identity.
pub fn acquire<T: Copy>(_control: Option<(&CLongAtomic, T)>) -> Option<T> {
    todo!()
}

/// Return a retained weak control, preserving its identity.
pub fn weak<T: Copy>(_control: Option<(&CLongAtomic, T)>) -> Option<T> {
    todo!()
}

//! Generic weak-object reference operations; counts are biased by one.
use obs_util::platform::CLongAtomic;

/// Retain a nullable weak handle.
pub fn addref(_weak: Option<&CLongAtomic>) { todo!() }

/// Release a nullable weak handle, returning whether its allocation must be freed.
pub fn release(_weak: Option<&CLongAtomic>) -> bool { todo!() }

/// Acquire a weak handle without testing the strong count for expiry.
pub fn get_weak<T: Copy>(_control: Option<(T, &CLongAtomic)>) -> Option<T> { todo!() }

/// Promote a weak handle only while the strong count is nonnegative.
pub fn get_object<T: Copy>(_weak: Option<(&CLongAtomic, T)>) -> Option<T> { todo!() }

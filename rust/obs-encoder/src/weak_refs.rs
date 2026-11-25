//! Encoder weak references use libobs's biased native-long atomic count.
use obs_util::platform::CLongAtomic;

/// Retain a nullable weak control.
pub fn addref(_count: Option<&CLongAtomic>) {
    todo!()
}

/// Release a nullable control; true transfers its allocation back to the allocator.
pub fn release(_count: Option<&CLongAtomic>) -> bool {
    todo!()
}

/// Acquire the stored control without inspecting the strong count or encoder.
pub fn get_weak<T: Copy>(_control: Option<(T, &CLongAtomic)>) -> Option<T> {
    todo!()
}

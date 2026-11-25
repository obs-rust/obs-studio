//! Encoder weak references use libobs's biased native-long atomic count.
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

/// Retain a nullable weak control.
pub fn addref(count: Option<&CLongAtomic>) {
    if let Some(count) = count {
        count.fetch_add(1, SeqCst);
    }
}

/// Release a nullable control; true transfers its allocation back to the allocator.
pub fn release(count: Option<&CLongAtomic>) -> bool {
    count.is_some_and(|count| count.fetch_sub(1, SeqCst) == 0)
}

/// Acquire the stored control without inspecting the strong count or encoder.
pub fn get_weak<T: Copy>(control: Option<(T, &CLongAtomic)>) -> Option<T> {
    control.map(|(handle, count)| {
        addref(Some(count));
        handle
    })
}

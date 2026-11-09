//! Generic weak-object reference operations; counts are biased by one.
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

/// Retain a nullable weak handle.
pub fn addref(weak: Option<&CLongAtomic>) {
    if let Some(weak) = weak {
        weak.fetch_add(1, SeqCst);
    }
}

/// Release a nullable weak handle, returning whether its allocation must be freed.
pub fn release(weak: Option<&CLongAtomic>) -> bool {
    weak.is_some_and(|weak| weak.fetch_sub(1, SeqCst) == 0)
}

/// Acquire a weak handle without testing the strong count for expiry.
pub fn get_weak<T: Copy>(control: Option<(T, &CLongAtomic)>) -> Option<T> {
    control.map(|(handle, weak)| {
        addref(Some(weak));
        handle
    })
}

/// Promote a weak handle only while the strong count is nonnegative.
pub fn get_object<T: Copy>(weak: Option<(&CLongAtomic, T)>) -> Option<T> {
    weak.and_then(|(refs, object)| {
        refs.fetch_update(SeqCst, SeqCst, |owners| {
            (owners >= 0).then(|| owners.wrapping_add(1))
        })
        .ok()
        .map(|_| object)
    })
}

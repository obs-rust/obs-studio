//! Canvas reference counts, biased by one: zero is one owner, -1 is expired.
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

/// Retain a weak reference; a missing handle is a no-op.
pub fn addref(count: Option<&CLongAtomic>) {
    if let Some(count) = count {
        count.fetch_add(1, SeqCst);
    }
}

/// Release a weak reference, returning whether its allocation must be freed.
pub fn release(count: Option<&CLongAtomic>) -> bool {
    count.is_some_and(|count| count.fetch_sub(1, SeqCst) == 0)
}

/// Upgrade a live control and preserve its target identity.
pub fn acquire<T: Copy>(control: Option<(&CLongAtomic, T)>) -> Option<T> {
    let (count, target) = control?;
    count
        .fetch_update(SeqCst, SeqCst, |owners| {
            (owners >= 0).then(|| owners.wrapping_add(1))
        })
        .ok()
        .map(|_| target)
}

/// Return a retained weak control, preserving its identity.
pub fn weak<T: Copy>(control: Option<(&CLongAtomic, T)>) -> Option<T> {
    let (count, identity) = control?;
    addref(Some(count));
    Some(identity)
}

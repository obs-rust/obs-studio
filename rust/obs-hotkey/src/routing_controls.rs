//! Configure callback routing while holding the registry's recursive mutex.

/// Storage supplied by the registry, accessed only after locking succeeds.
pub trait RoutingControls {
    /// Callback representation owned by the registry.
    type Callback;
    /// Caller-owned callback context.
    type Data;
    /// Acquire the registry; false means the core is absent or locking failed.
    fn lock(&mut self) -> bool;
    /// Replace the callback and its context under the acquired lock.
    fn set_callback(&mut self, callback: Self::Callback, data: Self::Data);
    /// Replace only the rerouting flag under the acquired lock.
    fn set_enabled(&mut self, enabled: bool);
    /// Release one successful acquisition.
    fn unlock(&mut self);
}

/// Store the router and context together, without invoking the router.
pub fn set_callback<R: RoutingControls>(registry: &mut R, callback: R::Callback, data: R::Data) {
    let _ = (registry, callback, data);
    todo!()
}

/// Enable or disable routing without changing the stored router or context.
pub fn set_enabled<R: RoutingControls>(registry: &mut R, enabled: bool) {
    let _ = (registry, enabled);
    todo!()
}

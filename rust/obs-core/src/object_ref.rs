//! Common object ownership, independent of the object's type tag.
use obs_util::platform::CLongAtomic;

/// Upgrade a live control, returning its stored identity (which may be NULL).
pub fn get_ref<T: Copy>(control: Option<(&CLongAtomic, T)>) -> Option<T> {
    crate::canvas_ref::acquire(control)
}

/// Private context access and lifetime effects supplied by the boundary.
pub trait Host {
    type Control: Copy;
    fn initialized(&self) -> bool;
    fn warn_shutdown(&mut self);
    fn control(&self) -> Option<Self::Control>;
    fn strong(&self, control: Self::Control) -> &CLongAtomic;
    fn destroy(&mut self);
    fn release_weak(&mut self, control: Self::Control);
}

/// Release the strong owner, destroying before releasing its weak owner.
pub fn release<H: Host>(host: &mut H) {
    if !host.initialized() {
        host.warn_shutdown();
        return;
    }
    let Some(control) = host.control() else {
        return;
    };
    if crate::canvas_ref::release(Some(host.strong(control))) {
        host.destroy();
        host.release_weak(control);
    }
}

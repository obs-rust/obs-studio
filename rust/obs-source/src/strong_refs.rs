//! Source strong-reference counting: `obs_source_addref()`,
//! `obs_source_release()`, `obs_source_get_ref()` and
//! `obs_weak_source_get_source()`. The strong count (`refs` of the source's
//! control) is biased by one: 0 is the last owner, the release that takes it
//! to -1 destroys the source, and a control at -1 is never promoted again.
use obs_util::ffi::tsan;
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

/// One `SeqCst` read-modify-write on the count, told to ThreadSanitizer as
/// the acquire-release that the C `os_atomic_*` on the same count gave it
/// (#5874). The tsan pass does not instrument Rust (#3507): without this,
/// the final release's `destroy()` races every earlier owner's accesses.
pub fn counted<R>(refs: &CLongAtomic, op: impl FnOnce(&CLongAtomic) -> R) -> R {
    tsan::release(refs);
    let result = op(refs);
    tsan::acquire(refs);
    result
}

/// The C steps around the count, one method each.
pub trait Host {
    /// A source (never NULL: the entries reject NULL first).
    type Source: Copy;
    /// A source's control (`obs_weak_source_t`).
    type Control: Copy;
    /// Whether the core is up (`obs != NULL`).
    fn core_alive(&mut self) -> bool;
    /// The shutdown warning, once.
    fn warn_shutdown(&mut self);
    /// The source's control; `None` for a NULL one, which no live source
    /// has.
    fn control(&mut self, source: Self::Source) -> Option<Self::Control>;
    /// The control's strong count.
    fn refs(&self, control: Self::Control) -> &CLongAtomic;
    /// The source the control names (`weak->source`).
    fn source(&mut self, control: Self::Control) -> Self::Source;
    /// `obs_source_destroy(source)`.
    fn destroy(&mut self, source: Self::Source);
    /// `obs_weak_source_release(control)`: the source's own weak reference.
    fn release_weak(&mut self, control: Self::Control);
}

/// Add one strong reference; `None` (a NULL source) is a no-op.
pub fn addref(refs: Option<&CLongAtomic>) {
    if let Some(refs) = refs {
        counted(refs, |refs| refs.fetch_add(1, SeqCst));
    }
}

/// `obs_weak_ref_get_ref()`: add one owner only while the count is above -1,
/// retrying when another thread moved it in between. `true` on success.
pub fn try_get_ref(refs: &CLongAtomic) -> bool {
    counted(refs, |refs| {
        refs.fetch_update(SeqCst, SeqCst, |owners| {
            (owners > -1).then(|| owners.wrapping_add(1))
        })
        .is_ok()
    })
}

/// `obs_source_release()`. With the core shut down a source is left alone
/// after one warning. Otherwise one owner is dropped; the step from 0 to -1
/// destroys the source and then drops its own weak reference, the control
/// read before the destroy.
pub fn release<H: Host>(host: &mut H, source: Option<H::Source>) {
    let Some(source) = source else {
        return;
    };
    if !host.core_alive() {
        host.warn_shutdown();
        return;
    }
    // A source without a control is impossible; the original would crash.
    let Some(control) = host.control(source) else {
        return;
    };
    if counted(host.refs(control), |refs| refs.fetch_sub(1, SeqCst)) == 0 {
        host.destroy(source);
        host.release_weak(control);
    }
}

/// `obs_weak_source_get_source()`: the control's source with one more owner,
/// or `None` for a NULL control or a source being destroyed.
pub fn weak_get_source<H: Host>(host: &mut H, weak: Option<H::Control>) -> Option<H::Source> {
    let weak = weak?;
    if try_get_ref(host.refs(weak)) {
        Some(host.source(weak))
    } else {
        None
    }
}

/// `obs_source_get_ref()`: the promotion of the source's own control.
pub fn get_ref<H: Host>(host: &mut H, source: Option<H::Source>) -> Option<H::Source> {
    let source = source?;
    let control = host.control(source);
    weak_get_source(host, control)
}

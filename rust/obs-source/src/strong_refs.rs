//! Source strong-reference counting: `obs_source_addref()`,
//! `obs_source_release()`, `obs_source_get_ref()` and
//! `obs_weak_source_get_source()`. The strong count (`refs` of the source's
//! control) is biased by one: 0 is the last owner, the release that takes it
//! to -1 destroys the source, and a control at -1 is never promoted again.
use obs_util::platform::CLongAtomic;

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
pub fn addref(_refs: Option<&CLongAtomic>) {
    todo!()
}

/// `obs_weak_ref_get_ref()`: add one owner only while the count is above -1,
/// retrying when another thread moved it in between. `true` on success.
pub fn try_get_ref(_refs: &CLongAtomic) -> bool {
    todo!()
}

/// `obs_source_release()`. With the core shut down a source is left alone
/// after one warning. Otherwise one owner is dropped; the step from 0 to -1
/// destroys the source and then drops its own weak reference, the control
/// read before the destroy.
pub fn release<H: Host>(_host: &mut H, _source: Option<H::Source>) {
    todo!()
}

/// `obs_weak_source_get_source()`: the control's source with one more owner,
/// or `None` for a NULL control or a source being destroyed.
pub fn weak_get_source<H: Host>(_host: &mut H, _weak: Option<H::Control>) -> Option<H::Source> {
    todo!()
}

/// `obs_source_get_ref()`: the promotion of the source's own control.
pub fn get_ref<H: Host>(_host: &mut H, _source: Option<H::Source>) -> Option<H::Source> {
    todo!()
}

//! Weak output ownership policy over the existing C atomic operations.

/// Operations on a retained control; the host preserves its allocator and atomics.
pub trait Host {
    type Weak: Copy;
    type Output: Copy + Eq;
    fn control(&self, output: Self::Output) -> Option<Self::Weak>;
    fn addref(&self, weak: Self::Weak);
    fn release(&self, weak: Self::Weak) -> bool;
    fn free(&self, weak: Self::Weak);
    fn promote(&self, weak: Self::Weak) -> bool;
    fn target(&self, weak: Self::Weak) -> Option<Self::Output>;
}

/// Retain an optional weak owner.
pub fn addref<H: Host>(_host: &H, _weak: Option<H::Weak>) {
    todo!()
}

/// Return the allocation only after the final weak owner releases it.
pub fn release<H: Host>(_host: &H, _weak: Option<H::Weak>) {
    todo!()
}

/// Borrow the control from a live output and retain it.
pub fn get_weak<H: Host>(_host: &H, _output: Option<H::Output>) -> Option<H::Weak> {
    todo!()
}

/// Acquire a strong owner only while the control remains live.
pub fn get_output<H: Host>(_host: &H, _weak: Option<H::Weak>) -> Option<H::Output> {
    todo!()
}

/// Compare stored identity independently of whether promotion can succeed.
pub fn references<H: Host>(
    _host: &H,
    _weak: Option<H::Weak>,
    _output: Option<H::Output>,
) -> bool {
    todo!()
}

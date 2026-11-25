//! Canvas removal orchestration, preserving the C reads and ownership order.

/// Operations on borrowed canvases. A successful promotion is released once,
/// after synchronous signal delivery; flags must not be borrowed across callbacks.
pub trait Host {
    type Canvas: Copy;
    fn flags(&mut self, canvas: Self::Canvas) -> u32;
    fn retain(&mut self, canvas: Self::Canvas) -> Option<Self::Canvas>;
    fn set_flags(&mut self, canvas: Self::Canvas, flags: u32);
    fn signal(&mut self, canvas: Self::Canvas);
    fn release(&mut self, canvas: Self::Canvas);
}

/// Remove a non-null borrowed canvas according to the existing canvas rules.
pub fn remove<H: Host>(_host: &mut H, _canvas: H::Canvas) {
    todo!()
}

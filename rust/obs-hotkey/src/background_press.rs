//! Background press policy, with synchronous notification after unlocking.

/// Private state access under the existing hotkey seam lock. Notification
/// borrows its payload only for the duration of the synchronous call.
pub trait BackgroundPress {
    fn lock(&mut self) -> bool;
    fn disabled(&self) -> bool;
    fn set_disabled(&mut self, disabled: bool);
    fn unlock(&mut self);
    fn signal(&mut self, enabled: bool);
}

/// Store the inverted disable bit and notify exactly once per transition.
pub fn enable<B: BackgroundPress>(_background: &mut B, _enabled: bool) {
    todo!("hotkey background press Rust swap #5137")
}

//! Shared mute/monitor callback policy; the host retains source ownership.

/// One selected control on an opaque source.
pub trait Control {
    /// Read the current value, only for a press.
    fn enabled(&mut self) -> bool;
    /// Apply a required transition through the original source setter.
    fn set_enabled(&mut self, enabled: bool);
}

/// Return true only when a press changes the selected control.
pub fn event(control: &mut impl Control, target: bool, pressed: bool) -> bool {
    if !pressed || control.enabled() == target {
        return false;
    }
    control.set_enabled(target);
    true
}

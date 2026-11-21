//! The call of one hotkey event (`libobs/obs-hotkey-dispatch.c.h`): the
//! hotkey's own callback, or the frontend's router when callback rerouting is
//! on, and nothing when rerouting is on and no router is set.
//!
//! No lookup, no lock and no state change: the caller (the binding press
//! count, with the hotkey mutex held) has already chosen to dispatch. The
//! rerouting flag, the router slot, the callback ABI and the hotkey's fields
//! stay with the caller's core, behind [`Dispatch`].

/// Access to the C-owned rerouting choice and the two native calls.
pub trait Dispatch {
    /// The hotkey, passed through as it is; never read before a call
    /// branch is chosen.
    type Hotkey: Copy;
    /// `obs->hotkeys.reroute_hotkeys`.
    fn rerouting(&mut self) -> bool;
    /// Whether `obs->hotkeys.router_func` is set.
    fn has_router(&mut self) -> bool;
    /// The hotkey's own callback with its data, id, the hotkey and `pressed`.
    fn call(&mut self, hotkey: Self::Hotkey, pressed: bool);
    /// The router with its data, the hotkey's id and `pressed`.
    fn route(&mut self, hotkey: Self::Hotkey, pressed: bool);
}

/// Deliver one event: the callback when rerouting is off (an installed
/// router is ignored), else the router when one is set, else nothing. The
/// hotkey reaches a call only on a delivered path, so the no-router branch
/// never touches it.
pub fn dispatch<D: Dispatch>(core: &mut D, hotkey: D::Hotkey, pressed: bool) {
    if !core.rerouting() {
        core.call(hotkey, pressed);
    } else if core.has_router() {
        core.route(hotkey, pressed);
    }
}

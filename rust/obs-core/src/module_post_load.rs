//! Post-load dispatch over the existing live module-list traversal.
use crate::module_lists::Lists;

/// External host hook and optional plugin callbacks, without private C layouts.
pub trait PostLoad: Lists {
    type Callback;
    fn host(&mut self);
    fn callback(&self, module: Self::Module) -> Option<Self::Callback>;
    fn call(&mut self, callback: Self::Callback);
}

/// Run the host hook before reading the enabled list, then dispatch in list order.
/// Loaded/state flags do not filter members. Nodes remain live across callbacks.
pub fn post_load<L: PostLoad>(_lists: &mut L) {
    todo!("Rust obs_post_load_modules swap (#4674)");
}

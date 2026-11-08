//! Synchronized borrowed background application ID queries.

/// The existing hotkey lock and the value read while it is held.
pub trait BackgroundAppId {
    type Id: Copy;
    fn lock(&mut self) -> bool;
    fn app_id(&self) -> Self::Id;
    fn unlock(&mut self);
}

/// Return no value when the core cannot be locked; otherwise borrow its ID.
pub fn query<B: BackgroundAppId>(background: &mut B) -> Option<B::Id> {
    if !background.lock() {
        return None;
    }
    let id = background.app_id();
    background.unlock();
    Some(id)
}

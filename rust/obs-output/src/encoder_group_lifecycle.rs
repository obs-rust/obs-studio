//! Encoder-group ownership and the locked teardown handoff.
//!
//! The backend owns allocation, private C fields and native mutex primitives.
//! All lifecycle ordering and deferred destruction decisions live here.
pub trait Backend {
    type Group: Copy;
    fn allocate_zeroed(&mut self) -> Self::Group;
    fn init_mutex(&mut self, group: Self::Group) -> bool;
    fn free_group(&mut self, group: Self::Group);
    fn lock(&mut self, group: Self::Group);
    fn unlock(&mut self, group: Self::Group);
    fn destroy_mutex(&mut self, group: Self::Group);
    fn started(&mut self, group: Self::Group) -> u32;
    fn defer(&mut self, group: Self::Group);
    fn count(&mut self, group: Self::Group) -> usize;
    fn clear_back_pointer(&mut self, group: Self::Group, index: usize);
    fn release(&mut self, group: Self::Group, index: usize);
    fn free_list(&mut self, group: Self::Group);
}

pub fn create<B: Backend>(_backend: &mut B) -> Option<B::Group> {
    todo!()
}

/// The caller hands over a group whose mutex is already locked.
pub fn actually_destroy<B: Backend>(_backend: &mut B, _group: B::Group) {
    todo!()
}

pub fn destroy<B: Backend>(_backend: &mut B, _group: Option<B::Group>) {
    todo!()
}

//! Locked public unregistration dispatch; registry ownership and teardown stay C.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    Single,
    Pair,
}

pub trait Registry {
    fn lock(&mut self) -> bool;
    fn single(&mut self, id: usize);
    fn pair(&mut self, id: usize);
    fn unlock(&mut self);
}

pub fn unregister(_registry: &mut impl Registry, _id: usize, _removal: Removal) {
    todo!("hotkey unregistration entry swap")
}

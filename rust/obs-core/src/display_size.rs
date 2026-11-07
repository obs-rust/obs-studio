//! Display size protocol. The boundary supplies the existing C-owned lock and
//! field access; stores stay separate because output pointers may alias.

/// Operations on one display and its two output locations.
pub trait SizeAccess {
    fn set_width(&mut self, value: u32);
    fn set_height(&mut self, value: u32);
    fn has_display(&self) -> bool;
    fn lock(&mut self);
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn unlock(&mut self);
}

/// Zero outputs before inspecting the display, then copy under its own lock.
pub fn size(_access: &mut impl SizeAccess) {
    todo!()
}

//! Stored matrix and box-scale copies under the captured parent's lock.

/// Access to one stored field. Implementations retain the captured parent
/// independently of later changes to the item's parent.
pub trait FieldAccess<const N: usize> {
    type Parent;
    fn lock(&mut self) -> Self::Parent;
    fn read(&self) -> [u32; N];
    fn write(&mut self, bits: [u32; N]);
    fn unlock(&mut self, parent: Self::Parent);
}

/// A missing item is a no-op; live fields are copied bit for bit while locked.
pub fn copy_field<const N: usize>(access: Option<&mut impl FieldAccess<N>>) {
    let _ = access;
    todo!("scene matrix getter port")
}

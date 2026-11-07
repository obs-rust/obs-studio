//! Read-only stored fields from `libobs/obs-hotkey-object-getters.c.h`.
/// Return the stored description handle without reading its bytes.
pub fn description<H: Copy>(_value: &H) -> H {
    todo!()
}
/// Return the stored registerer category without resolving its object.
pub fn registerer_type(_value: &u32) -> u32 {
    todo!()
}
/// Return the stored registerer handle without retaining it.
pub fn registerer<H: Copy>(_value: &H) -> H {
    todo!()
}
/// Return the independent stored partner ID.
pub fn pair_partner_id(_value: &usize) -> usize {
    todo!()
}

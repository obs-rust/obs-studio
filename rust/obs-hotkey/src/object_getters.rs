//! Read-only stored fields from `libobs/obs-hotkey-object-getters.c.h`.
/// Return the stored description handle without reading its bytes.
pub fn description<H: Copy>(value: &H) -> H {
    *value
}
/// Return the stored registerer category without resolving its object.
pub fn registerer_type(value: &u32) -> u32 {
    *value
}
/// Return the stored registerer handle without retaining it.
pub fn registerer<H: Copy>(value: &H) -> H {
    *value
}
/// Return the independent stored partner ID.
pub fn pair_partner_id(value: &usize) -> usize {
    *value
}

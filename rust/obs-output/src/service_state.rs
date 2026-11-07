//! State accessors for `obs_service`; callers retain ownership of all links.

pub fn set_destroy(destroy: &mut bool) {
    *destroy = true;
}
pub fn is_destroying(destroy: bool) -> bool {
    destroy
}
pub fn is_active(active: bool) -> bool {
    active
}
pub fn set_active(slot: &mut bool, active: bool) {
    *slot = active;
}
pub fn has_output<T: Default + PartialEq>(output: T) -> bool {
    output != T::default()
}
pub fn clear_output<T: Default>(output: &mut T) {
    *output = T::default();
}
pub fn detach_from_output<T: Default>(service: &mut T) {
    *service = T::default();
}
pub fn get_control<T>(control: T) -> T {
    control
}

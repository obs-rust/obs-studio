//! State accessors for `obs_service`; callers retain ownership of all links.

pub fn set_destroy(_destroy: &mut bool) {
    todo!("#3840")
}
pub fn is_destroying(_destroy: bool) -> bool {
    todo!("#3840")
}
pub fn is_active(_active: bool) -> bool {
    todo!("#3840")
}
pub fn set_active(_slot: &mut bool, _active: bool) {
    todo!("#3840")
}
pub fn has_output<T: Default + PartialEq>(_output: T) -> bool {
    todo!("#3840")
}
pub fn clear_output<T: Default>(_output: &mut T) {
    todo!("#3840")
}
pub fn detach_from_output<T: Default>(_service: &mut T) {
    todo!("#3840")
}
pub fn get_control<T>(_control: T) -> T {
    todo!("#3840")
}

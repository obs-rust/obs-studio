//! Context validity depends only on its borrowed data pointer.

/// Missing contexts and empty data slots are invalid.
pub fn is_invalid<T>(_data: Option<*mut T>) -> bool {
    todo!("port obs_obj_invalid")
}

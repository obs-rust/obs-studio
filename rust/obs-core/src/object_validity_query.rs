//! Context validity depends only on the presence of its borrowed data pointer.

/// Missing contexts and empty data slots are invalid; never dereference data.
pub fn is_invalid<T>(data: Option<*mut T>) -> bool {
    data.is_none_or(|data| data.is_null())
}

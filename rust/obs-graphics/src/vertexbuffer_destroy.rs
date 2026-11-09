//! Dispatch destruction only with a context and a non-null buffer.
pub fn destroy<T>(buffer: Option<T>, callback: Option<impl FnOnce(T)>) {
    if let (Some(buffer), Some(callback)) = (buffer, callback) {
        callback(buffer);
    }
}

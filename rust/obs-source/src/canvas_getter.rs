//! Acquire an independently owned canvas from a source's borrowed weak token.
pub fn get<W, R>(weak: W, acquire: impl FnOnce(W) -> R) -> R {
    let _ = (weak, acquire);
    todo!("source canvas ownership query")
}

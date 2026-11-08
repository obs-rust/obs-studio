//! Read-only, byte-exact lookup over a borrowed module registry.
pub fn lookup<'a, T: Copy>(
    first: Option<T>,
    name: &[u8],
    fields: impl FnMut(T) -> (&'a [u8], Option<T>),
) -> Option<T> {
    let _ = (first, name, fields);
    todo!("module name lookup port")
}

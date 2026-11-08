//! Read-only, byte-exact lookup over a borrowed module registry.
pub fn lookup<'a, T: Copy>(
    mut first: Option<T>,
    name: &[u8],
    mut fields: impl FnMut(T) -> (&'a [u8], Option<T>),
) -> Option<T> {
    while let Some(module) = first {
        let (candidate, next) = fields(module);
        if candidate == name {
            return Some(module);
        }
        first = next;
    }
    None
}

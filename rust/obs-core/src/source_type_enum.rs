//! Registry bounds and ordered output writes, independent of the C layout.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    Versioned,
    Unversioned,
}

pub fn source<T>(
    count: usize,
    index: usize,
    read: impl FnOnce(usize) -> T,
    write: impl FnOnce(T),
) -> bool {
    let _ = (count, index, read, write);
    todo!()
}

pub fn input<T>(
    count: usize,
    index: usize,
    read: impl FnOnce(usize) -> (T, T),
    versioned: bool,
    unversioned: bool,
    write: impl FnMut(Output, T),
) -> bool {
    let _ = (count, index, read, versioned, unversioned, write);
    todo!()
}

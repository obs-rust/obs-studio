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
    if index >= count {
        return false;
    }
    write(read(index));
    true
}

pub fn input<T>(
    count: usize,
    index: usize,
    mut read: impl FnMut(usize, Output) -> T,
    versioned: bool,
    unversioned: bool,
    mut write: impl FnMut(Output, T),
) -> bool {
    if index >= count {
        return false;
    }
    if versioned {
        write(Output::Versioned, read(index, Output::Versioned));
    }
    if unversioned {
        write(Output::Unversioned, read(index, Output::Unversioned));
    }
    true
}

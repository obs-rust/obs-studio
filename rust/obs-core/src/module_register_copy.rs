//! Safe core of `libobs/obs-module-register-copy.c.h`: the registration
//! size rules.
//!
//! A plugin registers a source, output, encoder or service type with the
//! size of its info structure, which is shorter than libobs's when it was
//! built against an older header. `obs-module.c` copies only the fields
//! that lie wholly within that size ([`whole_size`]), reads a callback only
//! when it lies wholly within it ([`within`], for `HAS_VAL()` and the free
//! of the type data on an error), refuses a size of 0 and one past what
//! libobs supports ([`gate`], for `REGISTER_OBS_DEF()`), and warns about
//! and ignores a transition's `get_width` and `get_height`
//! ([`ignores_size`]).
//!
//! `obs-module.c` keeps every `offsetof()` and `sizeof()` (passed in, so
//! the rules hold for any pointer size), every read of the caller's
//! structure, the copy, the type lists, the loading module's lists, every
//! log message and the `free_type_data` call.
//!
//! Sizes and offsets are `usize`s, as the C's `size_t`. Nothing here
//! panics, indexes or overflows for any input.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::module_source_rules::size_supported;

/// `sizeof(uint32_t)`: the size of the 32-bit fields [`whole_size`] keeps.
pub const U32_SIZE: usize = 4;

/// `OBS_SOURCE_TYPE_TRANSITION`.
pub const SOURCE_TYPE_TRANSITION: u32 = 2;

/// What `REGISTER_OBS_DEF()` does with the caller's size
/// (`enum obs_module_register_copy_gate`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Copy the structure and register it.
    Copy,
    /// A size of 0: the registration came from outside `obs_module_load`.
    OutsideLoad,
    /// A size past the structure libobs knows.
    TooLarge,
}

/// Same contract as C `obs_module_register_copy_whole_size`: how much of
/// the caller's structure is copied. Every field is a pointer, as large as
/// and aligned to `ptr_size`, except the 32-bit fields at `u32_fields`. The
/// copy ends at `size` rounded down to a pointer, or, reading the list once
/// in order, at the end of a 32-bit field that starts at or after the copy's
/// end so far and lies wholly within `size`.
pub fn whole_size(size: usize, ptr_size: usize, u32_fields: &[usize]) -> usize {
    let mut whole = size.saturating_sub(size.checked_rem(ptr_size).unwrap_or(0));
    for &field in u32_fields {
        if field >= whole
            && let Some(end) = field.checked_add(U32_SIZE)
            && end <= size
        {
            whole = end;
        }
    }
    whole
}

/// Same contract as C `obs_module_register_copy_within`: the field at
/// `offset`, `field_size` bytes long, lies wholly within `size`.
pub fn within(offset: usize, field_size: usize, size: usize) -> bool {
    offset
        .checked_add(field_size)
        .is_some_and(|end| end <= size)
}

/// Same contract as C `obs_module_register_copy_gate`: a size of 0 is
/// outside `obs_module_load`, a size past `supported` is too large, any
/// other size is copied.
pub fn gate(size: usize, supported: usize) -> Gate {
    if size == 0 {
        Gate::OutsideLoad
    } else if !size_supported(size, supported) {
        Gate::TooLarge
    } else {
        Gate::Copy
    }
}

/// Same contract as C `obs_module_register_copy_ignores_size`: a source
/// type whose `get_width` and `get_height` are warned about and ignored (a
/// transition).
pub fn ignores_size(source_type: u32) -> bool {
    source_type == SOURCE_TYPE_TRANSITION
}

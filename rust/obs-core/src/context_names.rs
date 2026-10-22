//! Safe core of `libobs/obs-context-names.c.h`: the context names.
//!
//! Every source, scene, canvas, encoder, output and service gets its name
//! through [`name_dup`]: a copy of the name, or `"__unnamed%04lld"` from the
//! counter `obs.c` keeps (`obs->data.unnamed_index`) for a NULL or empty
//! name. [`deduplicate`] then makes a taken name unique for the name tables
//! of `obs.c`: `None` when the name is free, else `"<name> <n>"` with the
//! first free `n` from 2 up. Which names are taken is asked of the caller,
//! one name at a time, as the C asks its `exists()` callback: the name
//! itself, then `"<name> 2"`, `"<name> 3"`, ... until one is free.
//!
//! Names are bytes, not text: they come from scene collections,
//! obs-websocket and scripts, so they can hold `%` sequences or bytes that
//! are not UTF-8, and nothing here parses them. Nothing here panics,
//! indexes or overflows for any input.
//!
//! Kept as the C has it, characterized and not endorsed (#1788):
//!
//! - only the name itself decides whether a suffix is needed, so `"A 2"`
//!   renamed to `"A"` while `"A"` is taken stays `"A 2"`;
//! - an empty name is made unique like any other (`" 2"`); `obs.c` gives an
//!   empty name an unnamed name first, so no table holds `""`;
//! - the counter is printed signed, with at least four digits.
//!
//! `obs.c` keeps the rest: renaming to `""` takes the next unnamed name
//! without [`deduplicate`], and the counter is incremented without a lock.

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

/// The prefix of an unnamed name.
pub const UNNAMED_PREFIX: &[u8] = b"__unnamed";

/// What `"%s"` prints for a NULL string in the C libraries libobs is built
/// with (glibc, the macOS libc and the MSVC CRT).
pub const NULL_STRING: &[u8] = b"(null)";

/// One question to the caller's `exists()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lookup<'a> {
    /// The name [`deduplicate`] was given, as it was given (`None` for
    /// NULL). Always the first question.
    Name(Option<&'a [u8]>),
    /// A candidate, `"<name> <n>"`.
    Candidate(&'a [u8]),
}

/// `"__unnamed%04lld"`: the unnamed name for counter value `index`.
/// `%04lld` pads with zeros to four characters, the sign included, as
/// Rust's `{:04}` does.
pub fn unnamed_name(index: i64) -> Vec<u8> {
    let mut name = UNNAMED_PREFIX.to_vec();
    name.extend_from_slice(format!("{index:04}").as_bytes());
    name
}

/// `"%s %d"`: the candidate for `name` with suffix `suffix`. A NULL name is
/// printed as [`NULL_STRING`], as the C's `"%s"` prints it.
pub fn suffixed_name(name: Option<&[u8]>, suffix: i32) -> Vec<u8> {
    let mut candidate = name.unwrap_or(NULL_STRING).to_vec();
    candidate.push(b' ');
    candidate.extend_from_slice(suffix.to_string().as_bytes());
    candidate
}

/// `obs_context_name_dup()`: the name a context gets. `None` for a private
/// context without a name; [`unnamed_name`] of the counter, which then
/// counts up, for a NULL or empty name; else a copy of the name.
///
/// The C's `(*unnamed_index)++` at `LLONG_MAX` is signed overflow; it wraps
/// here, as in the C oracle (libobs counts from 0).
pub fn name_dup(name: Option<&[u8]>, private: bool, unnamed_index: &mut i64) -> Option<Vec<u8>> {
    if private && name.is_none() {
        return None;
    }

    match name {
        None | Some(b"") => {
            let index = *unnamed_index;
            *unnamed_index = index.wrapping_add(1);
            Some(unnamed_name(index))
        }
        Some(name) => Some(name.to_vec()),
    }
}

/// `obs_context_name_deduplicate()`: `None` when `exists` says the name is
/// free, else the first [`suffixed_name`] from 2 up that `exists` says is
/// free. `exists` is asked [`Lookup::Name`] first, then each candidate in
/// order, once each, and nothing after the free one.
///
/// The suffix is the C's `int suffix`, counted up with `suffix++`. Here it
/// wraps rather than panic past `INT_MAX`, where the C's signed overflow is
/// undefined; neither is reached.
pub fn deduplicate<F>(name: Option<&[u8]>, mut exists: F) -> Option<Vec<u8>>
where
    F: FnMut(Lookup<'_>) -> bool,
{
    let mut taken = exists(Lookup::Name(name));
    if !taken {
        return None;
    }

    let mut new_name = Vec::new();
    let mut suffix: i32 = 2;

    while taken {
        new_name = suffixed_name(name, suffix);
        suffix = suffix.wrapping_add(1);
        taken = exists(Lookup::Candidate(&new_name));
    }

    Some(new_name)
}

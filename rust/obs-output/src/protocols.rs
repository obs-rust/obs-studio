//! Safe core of `libobs/obs-output-protocols.c.h`: the output protocol-list
//! match.
//!
//! `obs_enum_output_types_with_protocol()` (still C) walks the registered
//! service outputs and calls its callback once for each segment of an
//! output's `;`-separated `protocols` string that equals the protocol;
//! [`count_matches`] is that count. The frontend picks the stream output
//! for a protocol from it.
//!
//! Both inputs are untrusted bytes: the lists come from output plugins and
//! the protocol from the stream service (`services.json`, or derived from a
//! custom server URL). They are compared as bytes, never as text, and
//! nothing here can panic.
//!
//! The count is byte for byte the C's, including what
//! `test_output_protocol_list_steps` marks "characterized, not endorsed": a
//! protocol listed twice counts twice, and the empty protocol matches each
//! empty segment that a `;` ends, but not the segment after a trailing `;`
//! (the C stops scanning at the NUL), and nothing in an empty list.

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

/// The segment separator.
pub const SEPARATOR: u8 = b';';

/// How many `;`-separated segments of `list` equal `protocol`, byte for
/// byte: whole segments, case-sensitive, nothing trimmed. A protocol listed
/// twice counts twice.
///
/// `list` and `protocol` are the bytes of the C strings before their NULs,
/// so neither holds a NUL as libobs calls it; any byte here, a NUL
/// included, is compared as it is. The segment after a trailing `;` is not
/// one (the C reaches the NUL there and stops), and an empty list has no
/// segment, so the empty protocol matches only the empty segments that a
/// `;` ends.
pub fn count_matches(list: &[u8], protocol: &[u8]) -> usize {
    if list.is_empty() {
        return 0;
    }
    // The C scans a segment only while it has not reached the NUL, so a
    // trailing ';' ends the last segment it compares.
    let scanned = list.strip_suffix(&[SEPARATOR]).unwrap_or(list);
    scanned
        .split(|&b| b == SEPARATOR)
        .filter(|segment| *segment == protocol)
        .count()
}

//! Safe core of `libobs/obs-nix-system-info.c.h`: the system info parsing
//! of `libobs/obs-nix.c.h`'s startup log.
//!
//! - [`is_name`], [`is_physical_id`], [`is_speed`], [`value`],
//!   [`physical_id`] and [`prints`]: `log_processor_info()`'s
//!   `/proc/cpuinfo` line rules (Linux);
//! - [`is_dmesg_cpu`] and [`dmesg_speed`]: `log_processor_speed()`'s
//!   `dmesg.boot` line rules (FreeBSD);
//! - [`total_mb`], [`free_mb`] and [`physmem_mb`]: `log_memory_info()`'s
//!   sizes, from `sysinfo()` and OpenBSD's `HW_PHYSMEM64`.
//!
//! A line or value is the bytes before its NUL, as `getline()` leaves it in
//! C, and positions are offsets into it, so the C keeps its own buffer and
//! every `dstr` copy. Each key check is its own function, so the three
//! checks of a cpuinfo line stay where they were in C. The C keeps the file
//! reads, the `dstr` copies, cuts and depads, `sysinfo()` and `sysctl()`,
//! the pointer checks and the logs. Nothing here keeps state, takes a lock,
//! reads past the slice it is given or calls libobs.
//!
//! The core is OS-free: it parses bytes and sizes, and the C picks which
//! rules a host runs.
//!
//! Characterized, not endorsed (`test_nix_system_info_steps`), and kept:
//! arm64 logs no CPU name or speed (its physical id stays -1, the starting
//! "last id"); a `dmesg.boot` speed with no `-` runs to the end of the line;
//! the memory products wrap in `u64` (only above 16 EiB).
//!
//! No intentional differences from the C. [`physical_id`] is `atoi()` as
//! glibc defines it (`strtol()`, saturated to `long`, then truncated to
//! `int`); C leaves an out-of-range `atoi()` undefined, so that case has no
//! portable C behaviour to match.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]
use core::ops::Range;

/// The keys, as the C compares them: by their own length.
const NAME: &[u8] = b"model name";
const PHYSICAL_ID: &[u8] = b"physical id";
const SPEED: &[u8] = b"cpu MHz";
const DMESG_CPU: &[u8] = b"CPU: ";

/// Bytes per MB, applied twice as the C's `/ 1024 / 1024`.
const KB: u64 = 1024;

/// Whether a cpuinfo line starts with `model name`: `strncmp(line,
/// "model name", 10) == 0`, so a longer key with that prefix also matches.
pub fn is_name(line: &[u8]) -> bool {
    line.starts_with(NAME)
}

/// Whether a cpuinfo line starts with `physical id`.
pub fn is_physical_id(line: &[u8]) -> bool {
    line.starts_with(PHYSICAL_ID)
}

/// Whether a cpuinfo line starts with `cpu MHz`.
pub fn is_speed(line: &[u8]) -> bool {
    line.starts_with(SPEED)
}

/// Where a cpuinfo line's value starts: the byte after its first `:`, or
/// `None` when the line has no `:` or ends there.
pub fn value(line: &[u8]) -> Option<usize> {
    let colon = line.iter().position(|&b| b == b':')?;
    let start = colon.checked_add(1)?;
    (start < line.len()).then_some(start)
}

/// `isspace()` in the C locale.
fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// A physical id value as `atoi()` reads it: leading C-locale white space,
/// an optional sign and decimal digits, 0 when there are none.
///
/// glibc's `atoi()` is `(int)strtol(s, NULL, 10)`: the digits saturate at
/// the 64-bit `long`'s bounds, and the result is truncated to `int`.
pub fn physical_id(value: &[u8]) -> i32 {
    let mut rest = value;
    while let Some((&b, tail)) = rest.split_first() {
        if !is_c_space(b) {
            break;
        }
        rest = tail;
    }
    let negative = rest.first() == Some(&b'-');
    if let Some((&(b'-' | b'+'), tail)) = rest.split_first() {
        rest = tail;
    }

    // The magnitude, saturated one past i64::MAX so that -2^63 stays exact.
    let limit = i64::MAX.unsigned_abs().saturating_add(1);
    let mut magnitude: u64 = 0;
    for &b in rest.iter().take_while(|b| b.is_ascii_digit()) {
        let digit = u64::from(b.wrapping_sub(b'0'));
        magnitude = magnitude
            .saturating_mul(10)
            .saturating_add(digit)
            .min(limit);
    }

    let long = if negative {
        0i64.checked_sub_unsigned(magnitude).unwrap_or(i64::MIN)
    } else {
        i64::try_from(magnitude).unwrap_or(i64::MAX)
    };
    // `(int)` of a `long`: keep the low 32 bits.
    u32::try_from(long.cast_unsigned() & u64::from(u32::MAX))
        .unwrap_or(0)
        .cast_signed()
}

/// Whether a cpuinfo line ends a block that is printed: a line that starts
/// with a newline, with a physical id other than the last one printed.
pub fn prints(line: &[u8], physical_id: i32, last_physical_id: i32) -> bool {
    line.first() == Some(&b'\n') && physical_id != last_physical_id
}

/// Whether a `dmesg.boot` line starts with `CPU: `.
pub fn is_dmesg_cpu(line: &[u8]) -> bool {
    line.starts_with(DMESG_CPU)
}

/// The speed of a `dmesg.boot` `CPU: ` line: from the byte after its last
/// `(` up to its first `-` (or the end of the line), or `None` when the
/// line has no `(` or ends there.
pub fn dmesg_speed(line: &[u8]) -> Option<Range<usize>> {
    let open = line.iter().rposition(|&b| b == b'(')?;
    let start = open.checked_add(1)?;
    let rest = line.get(start..).filter(|rest| !rest.is_empty())?;
    let len = rest.iter().position(|&b| b == b'-').unwrap_or(rest.len());
    Some(start..start.checked_add(len)?)
}

/// The total memory in MB: `totalram * mem_unit / 1024 / 1024` in `u64`.
pub fn total_mb(totalram: u64, mem_unit: u32) -> u64 {
    mb(totalram.wrapping_mul(u64::from(mem_unit)))
}

/// The free memory in MB: `(freeram + bufferram) * mem_unit / 1024 / 1024`
/// in `u64`.
pub fn free_mb(freeram: u64, bufferram: u64, mem_unit: u32) -> u64 {
    mb(freeram
        .wrapping_add(bufferram)
        .wrapping_mul(u64::from(mem_unit)))
}

/// OpenBSD's physical memory in MB: `mem / 1024 / 1024` in `i64`, truncated
/// toward zero.
pub fn physmem_mb(mem: i64) -> i64 {
    // 1024 is neither 0 nor -1, so neither division can fail or overflow.
    mem / 1024 / 1024
}

fn mb(bytes: u64) -> u64 {
    bytes / KB / KB
}

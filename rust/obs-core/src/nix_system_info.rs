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
#![allow(clippy::todo)]

use core::ops::Range;

/// Whether a cpuinfo line starts with `model name`: `strncmp(line,
/// "model name", 10) == 0`, so a longer key with that prefix also matches.
pub fn is_name(line: &[u8]) -> bool {
    let _ = line;
    todo!()
}

/// Whether a cpuinfo line starts with `physical id`.
pub fn is_physical_id(line: &[u8]) -> bool {
    let _ = line;
    todo!()
}

/// Whether a cpuinfo line starts with `cpu MHz`.
pub fn is_speed(line: &[u8]) -> bool {
    let _ = line;
    todo!()
}

/// Where a cpuinfo line's value starts: the byte after its first `:`, or
/// `None` when the line has no `:` or ends there.
pub fn value(line: &[u8]) -> Option<usize> {
    let _ = line;
    todo!()
}

/// A physical id value as `atoi()` reads it: leading C-locale white space,
/// an optional sign and decimal digits, 0 when there are none.
pub fn physical_id(value: &[u8]) -> i32 {
    let _ = value;
    todo!()
}

/// Whether a cpuinfo line ends a block that is printed: a line that starts
/// with a newline, with a physical id other than the last one printed.
pub fn prints(line: &[u8], physical_id: i32, last_physical_id: i32) -> bool {
    let _ = (line, physical_id, last_physical_id);
    todo!()
}

/// Whether a `dmesg.boot` line starts with `CPU: `.
pub fn is_dmesg_cpu(line: &[u8]) -> bool {
    let _ = line;
    todo!()
}

/// The speed of a `dmesg.boot` `CPU: ` line: from the byte after its last
/// `(` up to its first `-` (or the end of the line), or `None` when the
/// line has no `(` or ends there.
pub fn dmesg_speed(line: &[u8]) -> Option<Range<usize>> {
    let _ = line;
    todo!()
}

/// The total memory in MB: `totalram * mem_unit / 1024 / 1024` in `u64`.
pub fn total_mb(totalram: u64, mem_unit: u32) -> u64 {
    let _ = (totalram, mem_unit);
    todo!()
}

/// The free memory in MB: `(freeram + bufferram) * mem_unit / 1024 / 1024`
/// in `u64`.
pub fn free_mb(freeram: u64, bufferram: u64, mem_unit: u32) -> u64 {
    let _ = (freeram, bufferram, mem_unit);
    todo!()
}

/// OpenBSD's physical memory in MB: `mem / 1024 / 1024` in `i64`, truncated
/// toward zero.
pub fn physmem_mb(mem: i64) -> i64 {
    let _ = mem;
    todo!()
}

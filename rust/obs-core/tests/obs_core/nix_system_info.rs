//! Tier 1: `obs_core::nix_system_info`, the Linux and BSD system info
//! parsing.
//!
//! The `steps_*` cases port `test/cmocka/test_nix_system_info_steps.c` 1:1:
//! its 11 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones included.
//! The C calls the hidden `obs_nix_system_info_*` functions; here the safe
//! core is called directly, and the C's copies of the callers' loops use
//! obs-util's `Dstr` (the core of libobs's `dstr` calls), with a pointer
//! becoming an offset into the line and NULL becoming `None`.
//!
//! The `edge_*` cases are new: proptests over any line from the cpuinfo
//! alphabet, any number text and any memory sizes, against models written
//! from the C's expressions.

use core::ops::Range;

use obs_core::nix_system_info::{
    dmesg_speed, free_mb, is_dmesg_cpu, is_name, is_physical_id, is_speed, physical_id, physmem_mb,
    prints, total_mb, value,
};
use obs_util::dstr::Dstr;
use proptest::prelude::*;

/// A logged string: `None` where the C's `dstr` array is NULL.
fn logged(d: &Dstr) -> Option<Vec<u8>> {
    (d.capacity() != 0).then(|| d.as_bytes().to_vec())
}

#[derive(Debug, Default)]
struct Block {
    name: Option<Vec<u8>>,
    speed: Option<Vec<u8>>,
}

#[derive(Debug, Default)]
struct CpuinfoRun {
    blocks: Vec<Block>,
    name: Option<Vec<u8>>,
    speed: Option<Vec<u8>>,
    physical_id: i32,
}

/// The lines as `getline()` returns them: up to and including `\n`, or the
/// rest of the text.
fn lines(text: &[u8]) -> impl Iterator<Item = &[u8]> {
    text.split_inclusive(|&b| b == b'\n')
}

/// The C's copy, cut and depad of a value: `dstr_copy()`, then
/// `dstr_resize(len - 1)` and `dstr_depad()`.
fn copy_value(d: &mut Dstr, value: &[u8]) {
    d.copy(value);
    d.resize(d.len().wrapping_sub(1));
    d.depad();
}

/// `log_processor_info()`'s loop, logging into the run instead of `blog()`.
fn run_cpuinfo(text: &str) -> CpuinfoRun {
    let mut run = CpuinfoRun::default();
    let mut physical = -1;
    let mut last_physical = -1;
    let mut name = Dstr::new();
    let mut speed = Dstr::new();

    for line in lines(text.as_bytes()) {
        if is_name(line) {
            let Some(start) = value(line) else { continue };
            copy_value(&mut name, &line[start..]);
        }

        if is_physical_id(line) {
            let Some(start) = value(line) else { continue };
            physical = physical_id(&line[start..]);
        }

        if is_speed(line) {
            let Some(start) = value(line) else { continue };
            copy_value(&mut speed, &line[start..]);
        }

        if prints(line, physical, last_physical) {
            last_physical = physical;
            assert!(run.blocks.len() < 8);
            run.blocks.push(Block {
                name: logged(&name),
                speed: logged(&speed),
            });
        }
    }

    run.name = logged(&name);
    run.speed = logged(&speed);
    run.physical_id = physical;
    run
}

/// `log_processor_speed()`'s loop: the speed it would log, or `None`.
fn run_dmesg(text: &str) -> Option<Vec<u8>> {
    let mut speed = Dstr::new();
    for line in lines(text.as_bytes()) {
        if is_dmesg_cpu(line) {
            let Some(range) = dmesg_speed(line) else {
                continue;
            };
            speed.ncopy(&line[range]);
        }
    }
    logged(&speed)
}

fn assert_block(block: &Block, name: &str, speed: &str) {
    assert_eq!(block.name.as_deref(), Some(name.as_bytes()));
    assert_eq!(block.speed.as_deref(), Some(speed.as_bytes()));
}

fn some(s: &str) -> Option<Vec<u8>> {
    Some(s.as_bytes().to_vec())
}

const X86_TWO_SOCKETS: &str = "processor\t: 0\n\
    vendor_id\t: GenuineIntel\n\
    model name\t: Intel(R) Xeon(R) CPU E5-2680 v4 @ 2.40GHz\n\
    cpu MHz\t\t: 2394.454\n\
    physical id\t: 0\n\
    siblings\t: 2\n\
    \n\
    processor\t: 1\n\
    model name\t: Intel(R) Xeon(R) CPU E5-2680 v4 @ 2.40GHz\n\
    cpu MHz\t\t: 2401.000\n\
    physical id\t: 0\n\
    \n\
    processor\t: 2\n\
    model name\t: Intel(R) Xeon(R) CPU E5-2690 v4 @ 2.60GHz\n\
    cpu MHz\t\t: 2600.000\n\
    physical id\t: 1\n\
    \n\
    processor\t: 3\n\
    model name\t: Intel(R) Xeon(R) CPU E5-2690 v4 @ 2.60GHz\n\
    cpu MHz\t\t: 1200.000\n\
    physical id\t: 1\n\
    \n";

/// Mirrors `x86_two_sockets`: one block per physical id.
#[test]
fn steps_x86_two_sockets() {
    let run = run_cpuinfo(X86_TWO_SOCKETS);
    assert_eq!(run.blocks.len(), 2);
    // each block logs the values its last line set, here the first thread
    // of each socket
    assert_block(
        &run.blocks[0],
        "Intel(R) Xeon(R) CPU E5-2680 v4 @ 2.40GHz",
        "2394.454",
    );
    assert_block(
        &run.blocks[1],
        "Intel(R) Xeon(R) CPU E5-2690 v4 @ 2.60GHz",
        "2600.000",
    );
    assert_eq!(run.physical_id, 1);
}

/// Mirrors `x86_one_socket`.
#[test]
fn steps_x86_one_socket() {
    let run = run_cpuinfo(
        "model name\t: AMD Ryzen 9 6900HX with Radeon Graphics\n\
         physical id\t: 0\n\
         cpu MHz\t\t: 3300.000\n\
         \n\
         model name\t: AMD Ryzen 9 6900HX with Radeon Graphics\n\
         physical id\t: 0\n\
         cpu MHz\t\t: 400.000\n\
         \n",
    );
    assert_eq!(run.blocks.len(), 1);
    assert_block(
        &run.blocks[0],
        "AMD Ryzen 9 6900HX with Radeon Graphics",
        "3300.000",
    );
}

const ARM64: &str = "processor\t: 0\n\
    BogoMIPS\t: 48.00\n\
    Features\t: fp asimd evtstrm aes pmull sha1 sha2 crc32\n\
    CPU implementer\t: 0x41\n\
    CPU part\t: 0xd08\n\
    \n\
    processor\t: 1\n\
    BogoMIPS\t: 48.00\n\
    \n";

/// Mirrors `arm64_logs_nothing`. Characterized, not endorsed: arm64 has no
/// physical id and no model name, so the id stays -1, the starting "last
/// id", and nothing is logged.
#[test]
fn steps_arm64_logs_nothing() {
    let run = run_cpuinfo(ARM64);
    assert_eq!(run.blocks.len(), 0);
    assert_eq!(run.name, None);
    assert_eq!(run.speed, None);
    assert_eq!(run.physical_id, -1);
}

/// Mirrors `block_before_name`. Characterized, not endorsed: a block that
/// ends before any model name or cpu MHz line logs them as NULL.
#[test]
fn steps_block_before_name() {
    let run = run_cpuinfo(
        "physical id\t: 0\n\nmodel name\t: Late\nphysical id\t: 1\ncpu MHz\t\t: 1.0\n\n",
    );
    assert_eq!(run.blocks.len(), 2);
    assert_eq!(run.blocks[0].name, None);
    assert_eq!(run.blocks[0].speed, None);
    assert_block(&run.blocks[1], "Late", "1.0");
}

/// Mirrors `last_line_without_newline`. Characterized, not endorsed: the
/// caller cuts the last byte of every value as its newline, so a value on
/// the last line of a file with no newline loses its last real byte.
#[test]
fn steps_last_line_without_newline() {
    let run = run_cpuinfo("physical id\t: 0\ncpu MHz\t\t: 1234.567\nmodel name\t: Name");
    assert_eq!(run.blocks.len(), 0);
    assert_eq!(run.name, some("Nam"));
    assert_eq!(run.speed, some("1234.567"));

    let run = run_cpuinfo("model name\t: Name\ncpu MHz\t\t: 1234.567");
    assert_eq!(run.name, some("Name"));
    assert_eq!(run.speed, some("1234.56"));
}

/// Mirrors `key_prefixes`.
#[test]
fn steps_key_prefixes() {
    assert!(is_name(b"model name\t: x\n"));
    assert!(is_name(b"model name"));
    // compared by the key's length, so a longer key matches
    assert!(is_name(b"model names\t: x\n"));
    assert!(is_name(b"model namex"));
    assert!(!is_name(b"model nam"));
    assert!(!is_name(b"Model name\t: x\n"));
    assert!(!is_name(b" model name\t: x\n"));
    assert!(!is_name(b"\n"));
    assert!(!is_name(b""));

    assert!(is_physical_id(b"physical id\t: 0\n"));
    assert!(is_physical_id(b"physical ids\t: 0\n"));
    assert!(!is_physical_id(b"physical i"));
    assert!(!is_physical_id(b"physical_id\t: 0\n"));

    assert!(is_speed(b"cpu MHz\t\t: 1.0\n"));
    assert!(is_speed(b"cpu MHz dynamic\t: 1.0\n"));
    assert!(!is_speed(b"cpu MH"));
    assert!(!is_speed(b"CPU MHz\t\t: 1.0\n"));
    assert!(!is_speed(b"cpu mhz\t\t: 1.0\n"));

    // the keys never overlap
    assert!(!is_physical_id(b"model name\t: x\n"));
    assert!(!is_speed(b"model name\t: x\n"));
    assert!(!is_name(b"cpu MHz\t\t: 1.0\n"));
}

/// Mirrors `values`.
#[test]
fn steps_values() {
    assert_eq!(value(b"model name\t: Intel\n"), Some(12));
    // the first ':', not the last
    assert_eq!(value(b"model name\t: a:b\n"), Some(12));
    assert_eq!(value(b"model name:x"), Some(11));
    // no ':'
    assert_eq!(value(b"model name\n"), None);
    assert_eq!(value(b""), None);
    // a line that ends at the ':' (the last line, with no newline)
    assert_eq!(value(b"model name\t:"), None);
    // characterized, not endorsed: "model name\t:\n" has the newline as its
    // value, so the caller cuts it to nothing and dstr_resize() frees the
    // name
    assert_eq!(value(b"model name\t:\n"), Some(12));

    let run = run_cpuinfo("model name\t: Old\nmodel name\t:\nphysical id\t: 0\n\n");
    assert_eq!(run.blocks.len(), 1);
    assert_eq!(run.blocks[0].name, None);

    // "model name\t:" at the end of the file keeps the old name
    let run = run_cpuinfo("model name\t: Old\nmodel name\t:");
    assert_eq!(run.name, some("Old"));

    // depadded on both ends after the cut
    let run = run_cpuinfo(
        "model name\t:  \t Padded  Name \t\nphysical id\t: 3\ncpu MHz\t:\t 800.0  \n\n",
    );
    assert_eq!(run.blocks.len(), 1);
    assert_block(&run.blocks[0], "Padded  Name", "800.0");
}

/// Mirrors `physical_ids`.
#[test]
fn steps_physical_ids() {
    assert_eq!(physical_id(b" 0\n"), 0);
    assert_eq!(physical_id(b" 1\n"), 1);
    assert_eq!(physical_id(b"\t 12\n"), 12);
    assert_eq!(physical_id(b" -3\n"), -3);
    assert_eq!(physical_id(b" +7\n"), 7);
    assert_eq!(physical_id(b" 42abc\n"), 42);
    assert_eq!(physical_id(b" 2147483647\n"), i32::MAX);
    assert_eq!(physical_id(b" -2147483648\n"), i32::MIN);
    // not a number
    assert_eq!(physical_id(b" x\n"), 0);
    assert_eq!(physical_id(b" - 1\n"), 0);
    assert_eq!(physical_id(b"\n"), 0);
    assert_eq!(physical_id(b" 0x10\n"), 0);

    // characterized, not endorsed: "physical id : x" reads as 0, so a block
    // after it is logged as socket 0
    let run = run_cpuinfo("physical id : x\nmodel name : X\n\n");
    assert_eq!(run.blocks.len(), 1);
    assert_eq!(run.physical_id, 0);

    // a "physical id" line with no value keeps the old id
    let run = run_cpuinfo("physical id\t: 5\nphysical id\t:");
    assert_eq!(run.physical_id, 5);
}

/// Mirrors `print_rule`.
#[test]
fn steps_print_rule() {
    assert!(prints(b"\n", 0, -1));
    assert!(prints(b"\n", 1, 0));
    assert!(prints(b"\n", -1, 0));
    assert!(prints(b"\n", i32::MIN, i32::MAX));
    assert!(!prints(b"\n", -1, -1));
    assert!(!prints(b"\n", 0, 0));
    // only a line that starts with the newline
    assert!(!prints(b" \n", 0, -1));
    assert!(!prints(b"\r\n", 0, -1));
    assert!(!prints(b"\t\n", 0, -1));
    assert!(!prints(b"", 0, -1));
    assert!(!prints(b"x\n", 0, -1));
    assert!(prints(b"\nx", 0, -1));
}

/// Mirrors `dmesg_lines`. The C's pointer and `*len` are the range here; a
/// NULL return leaves `*len` untouched in C, which `None` cannot change.
#[test]
fn steps_dmesg_lines() {
    assert!(is_dmesg_cpu(b"CPU: AMD\n"));
    assert!(is_dmesg_cpu(b"CPU: "));
    assert!(!is_dmesg_cpu(b"CPU:"));
    assert!(!is_dmesg_cpu(b"CPU:x"));
    assert!(!is_dmesg_cpu(b"cpu: x"));
    assert!(!is_dmesg_cpu(b" CPU: x"));

    let line = b"CPU: AMD Ryzen 7 5800X 8-Core Processor (3800.11-MHz K8-class CPU)\n";
    let open = line.iter().position(|&b| b == b'(').unwrap() + 1;
    assert_eq!(dmesg_speed(line), Some(open..open + 7));
    // the last '('
    assert_eq!(dmesg_speed(b"CPU: a (1-b (2-c\n"), Some(13..14));
    // no '('
    assert_eq!(dmesg_speed(b"CPU: none\n"), None);
    // a line that ends at the '('
    assert_eq!(dmesg_speed(b"CPU: x ("), None);
    // characterized, not endorsed: with no '-', the rest of the line is the
    // speed, its newline and ')' included
    assert_eq!(dmesg_speed(b"CPU: x (500)\n"), Some(8..13));
    assert_eq!(dmesg_speed(b"CPU: x (\n"), Some(8..9));
    // a '-' first gives an empty speed
    assert_eq!(dmesg_speed(b"CPU: x (-MHz)\n"), Some(8..8));

    assert_eq!(
        run_dmesg(
            "Copyright (c) 1992-2023 The FreeBSD Project.\n\
             CPU: AMD Ryzen 7 5800X 8-Core Processor (3800.11-MHz K8-class CPU)\n  \
             Origin=\"AuthenticAMD\"  Id=0xa20f12\n"
        ),
        some("3800.11")
    );

    // the last CPU line with a speed wins; an empty one clears it
    assert_eq!(
        run_dmesg("CPU: A (1000.00-MHz)\nCPU: B (2000.00-MHz)\nCPU: none\n"),
        some("2000.00")
    );
    assert_eq!(run_dmesg("CPU: A (1000.00-MHz)\nCPU: B (-MHz)\n"), None);
    assert_eq!(run_dmesg("Copyright (c) 1992-2023\n"), None);
}

/// Mirrors `memory_sizes`.
#[test]
fn steps_memory_sizes() {
    // mem_unit 1 (64-bit kernels)
    assert_eq!(total_mb(17_179_869_184, 1), 16384);
    assert_eq!(total_mb(1_048_575, 1), 0);
    assert_eq!(total_mb(1_048_576, 1), 1);
    assert_eq!(free_mb(1_048_576, 1_048_576, 1), 2);
    assert_eq!(free_mb(524_288, 524_287, 1), 0);
    assert_eq!(free_mb(524_288, 524_288, 1), 1);
    // mem_unit 4096 (pages), totalram near 2^32
    assert_eq!(total_mb(u64::from(u32::MAX), 4096), 16_777_215);
    assert_eq!(total_mb(4_294_967_296, 4096), 16_777_216);
    assert_eq!(total_mb(256, 4096), 1);
    assert_eq!(total_mb(255, 4096), 0);
    assert_eq!(
        free_mb(u64::from(u32::MAX), u64::from(u32::MAX), 4096),
        33_554_431
    );
    assert_eq!(free_mb(128, 128, 4096), 1);
    // mem_unit 0
    assert_eq!(total_mb(u64::MAX, 0), 0);
    assert_eq!(free_mb(u64::MAX, u64::MAX, 0), 0);
    // characterized, not endorsed: the products wrap in uint64_t (only above
    // 16 EiB)
    assert_eq!(total_mb(1 << 63, 2), 0);
    assert_eq!(total_mb(u64::MAX, u32::MAX), 17_592_186_040_320);
    assert_eq!(free_mb(u64::MAX, 1, 4096), 0);
    assert_eq!(free_mb(u64::MAX, 2, 1), 0);

    // OpenBSD: signed, truncated toward zero
    assert_eq!(physmem_mb(17_179_869_184), 16384);
    assert_eq!(physmem_mb(0), 0);
    assert_eq!(physmem_mb(1_048_575), 0);
    assert_eq!(physmem_mb(-1), 0);
    assert_eq!(physmem_mb(-1_048_576), -1);
    assert_eq!(physmem_mb(-2_097_151), -1);
    assert_eq!(physmem_mb(i64::MAX), 8_796_093_022_207);
    assert_eq!(physmem_mb(i64::MIN), -8_796_093_022_208);
}

/// The pieces of the cpuinfo and dmesg.boot alphabet the proptests join.
const PIECES: &[&[u8]] = &[
    b"model name",
    b"model names",
    b"model nam",
    b"physical id",
    b"physical i",
    b"cpu MHz",
    b"cpu MH",
    b"CPU: ",
    b"CPU:",
    b":",
    b" : ",
    b"\t",
    b" ",
    b"\n",
    b"\r",
    b"\x0b",
    b"\x0c",
    b"-",
    b"+",
    b"(",
    b")",
    b"0",
    b"7",
    b"42",
    b"2147483647",
    b"2147483648",
    b"99999999999999999999",
    b"x",
    "\u{e9}".as_bytes(),
    b"\xff",
];

fn line_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(prop::sample::select(PIECES), 0..8).prop_map(|p| p.concat())
}

/// `strncmp(line, key, key.len()) == 0`, as the C compares a key.
fn model_key(line: &[u8], key: &[u8]) -> bool {
    line.len() >= key.len() && line[..key.len()] == *key
}

/// `strchr()`, then the byte after it unless that is the end.
fn model_value(line: &[u8]) -> Option<usize> {
    let colon = line.iter().position(|&b| b == b':')?;
    (colon + 1 < line.len()).then_some(colon + 1)
}

/// glibc's `atoi()`: `(int)strtol(s, NULL, 10)`, with `long` 64 bits.
fn model_atoi(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && matches!(s[i], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        i += 1;
    }
    let negative = i < s.len() && s[i] == b'-';
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        i += 1;
    }
    let mut n: i128 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        n = (n * 10 + i128::from(s[i] - b'0')).min(i128::from(i64::MAX) + 1);
        i += 1;
    }
    let n = if negative { -n } else { n };
    let n = n.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
    n as i32
}

/// `strrchr()`, then the byte after it unless that is the end, and
/// `strcspn(start, "-")`.
fn model_dmesg(line: &[u8]) -> Option<Range<usize>> {
    let open = line.iter().rposition(|&b| b == b'(')?;
    let start = open + 1;
    if start >= line.len() {
        return None;
    }
    let len = line[start..]
        .iter()
        .position(|&b| b == b'-')
        .unwrap_or(line.len() - start);
    Some(start..start + len)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// Every line rule over any line from the cpuinfo and dmesg.boot
    /// alphabet, against the C's expressions.
    #[test]
    fn edge_line_rules_any_line(line in line_strategy(), id in any::<i32>(), last in any::<i32>()) {
        prop_assert_eq!(is_name(&line), model_key(&line, b"model name"));
        prop_assert_eq!(is_physical_id(&line), model_key(&line, b"physical id"));
        prop_assert_eq!(is_speed(&line), model_key(&line, b"cpu MHz"));
        prop_assert_eq!(is_dmesg_cpu(&line), model_key(&line, b"CPU: "));
        let start = value(&line);
        prop_assert_eq!(start, model_value(&line));
        if let Some(start) = start {
            prop_assert_eq!(physical_id(&line[start..]), model_atoi(&line[start..]));
        }
        prop_assert_eq!(prints(&line, id, last), line.first() == Some(&b'\n') && id != last);
        prop_assert_eq!(prints(&line, id, id), false);
        prop_assert_eq!(dmesg_speed(&line), model_dmesg(&line));
    }

    /// `atoi()` over any number text glibc reads, out-of-range values
    /// included.
    #[test]
    fn edge_physical_id_any_number(
        space in "[ \t\n\x0b\x0c\r]{0,3}",
        sign in prop::sample::select(&["", "-", "+", "--", "+-"][..]),
        digits in "[0-9]{0,24}",
        tail in "[0-9a-z: \n-]{0,4}",
    ) {
        let text = format!("{space}{sign}{digits}{tail}");
        prop_assert_eq!(physical_id(text.as_bytes()), model_atoi(text.as_bytes()));
    }

    /// The memory sizes over any `u64` fields and any `mem_unit`.
    #[test]
    fn edge_memory_any_size(
        total in any::<u64>(),
        free in any::<u64>(),
        buffer in any::<u64>(),
        unit in any::<u32>(),
        mem in any::<i64>(),
    ) {
        let unit64 = u64::from(unit);
        prop_assert_eq!(total_mb(total, unit), total.wrapping_mul(unit64) / 1024 / 1024);
        prop_assert_eq!(
            free_mb(free, buffer, unit),
            free.wrapping_add(buffer).wrapping_mul(unit64) / 1024 / 1024
        );
        prop_assert_eq!(physmem_mb(mem), mem / 1024 / 1024);
    }
}

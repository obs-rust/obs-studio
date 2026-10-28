//! Tier 1: `obs_core::win_crash_report`, the Windows crash report rules.
//!
//! The `steps_*` cases port `test/cmocka/test_win_crash_report_steps.c` 1:1:
//! its 8 cases under the same names, in the same order, with the same values
//! and expectations, the "characterized, not endorsed" ones included. The C
//! calls the hidden `obs_win_crash_report_*` functions; here the safe core is
//! called directly, and the C's copy of `write_thread_traces()`'s two passes
//! is ported with it. Where the C passes NULL for the re-entry flag (a
//! pointer it must never read), the closure here panics.
//!
//! The `edge_*` cases are new: proptests over any address, module, thread,
//! flags and value, against models written from the C's expressions.

use obs_core::win_crash_report::{
    SYMFLAG_EXPORT, contains, in_pass, is_crashed, mask_address, mask_param, name_prefix,
    names_symbol, other_process, pass_on,
};
use proptest::prelude::*;

#[test]
fn steps_module_edges() {
    let base: u64 = 0x7FF6_1234_0000;
    let size: u32 = 0x2000;
    let end = base + u64::from(size);

    assert!(!contains(base - 1, base, size));
    assert!(contains(base, base, size));
    assert!(contains(end - 1, base, size));
    assert!(!contains(end, base, size));
    assert!(!contains(0, base, size));
    assert!(!contains(u64::MAX, base, size));

    // a module of size 0 holds nothing, not even its base
    assert!(!contains(base, base, 0));

    // a module at 0, and the largest size
    assert!(contains(0, 0, 1));
    assert!(!contains(1, 0, 1));
    assert!(contains(0xFFFF_FFFE, 0, u32::MAX));
    assert!(!contains(0xFFFF_FFFF, 0, u32::MAX));

    // a 32-bit build's addresses
    assert!(contains(0x0040_1000, 0x0040_0000, 0x10000));
    assert!(!contains(0x0041_0000, 0x0040_0000, 0x10000));
}

#[test]
fn steps_module_end_wraps() {
    // characterized, not endorsed: base + size is computed in uint64_t, so
    // a module that ends past 2^64 wraps and holds nothing
    let base = u64::MAX - 15;

    assert!(!contains(base, base, 32));
    assert!(!contains(u64::MAX, base, 32));
    assert!(!contains(5, base, 32));
    // one that ends at exactly 2^64 too: its end wraps to 0
    assert!(!contains(base, base, 16));
    assert!(!contains(u64::MAX, base, 16));
    // one byte short of it does not wrap
    assert!(contains(base, base, 15));
    assert!(contains(u64::MAX - 1, base, 15));
    assert!(!contains(u64::MAX, base, 15));
}

#[test]
fn steps_thread_rules() {
    assert!(is_crashed(0x1A2C, 0x1A2C));
    assert!(!is_crashed(0x1A2C, 0x1A2D));
    assert!(!is_crashed(0, u32::MAX));
    assert!(is_crashed(u32::MAX, u32::MAX));

    // the crashed thread in the first pass, every other in the second
    assert!(in_pass(true, true));
    assert!(!in_pass(true, false));
    assert!(!in_pass(false, true));
    assert!(in_pass(false, false));

    assert!(!other_process(4242, 4242));
    assert!(other_process(4, 4242));
    assert!(other_process(u32::MAX, 0));
}

/// A snapshot entry: the thread's id and its owner process.
#[derive(Clone, Copy, Debug)]
struct Thread {
    id: u32,
    owner: u32,
}

const fn thread(id: u32, owner: u32) -> Thread {
    Thread { id, owner }
}

/// `write_thread_traces()`'s two passes over a snapshot, with
/// `write_thread_trace()`'s checks in their order: the ids the report
/// traces, and whether each is labeled "(Crashed)".
fn trace_threads(
    threads: &[Thread],
    current_thread: u32,
    current_process: u32,
) -> Vec<(u32, bool)> {
    let mut out = Vec::new();
    for first_thread in [true, false] {
        for t in threads {
            let crash_thread = is_crashed(t.id, current_thread);
            if !in_pass(first_thread, crash_thread) {
                continue;
            }
            if other_process(t.owner, current_process) {
                continue;
            }
            out.push((t.id, crash_thread));
        }
    }
    out
}

#[test]
fn steps_thread_passes() {
    // the crashed thread first, then the others in snapshot order, and
    // another process's threads left out
    let snapshot = [
        thread(0x10, 700),
        thread(0x24, 700),
        thread(0x31, 912),
        thread(0x18, 700),
        thread(0x2C, 700),
    ];
    assert_eq!(
        trace_threads(&snapshot, 0x18, 700),
        [(0x18, true), (0x10, false), (0x24, false), (0x2C, false)]
    );

    // no crashed thread in the snapshot: the first pass traces nothing
    assert_eq!(
        trace_threads(&snapshot, 0x99, 700),
        [(0x10, false), (0x24, false), (0x18, false), (0x2C, false)]
    );

    // the crashed thread's id under another process is that process's
    // thread: left out of both passes
    let other = [thread(0x18, 912), thread(0x10, 700)];
    assert_eq!(trace_threads(&other, 0x18, 700), [(0x10, false)]);

    // every thread in another process, and an empty snapshot
    assert_eq!(trace_threads(&snapshot, 0x18, 1), []);
    assert_eq!(trace_threads(&[], 0x18, 700), []);

    // the same id twice, the crashed one: traced twice in the first pass
    let twice = [thread(0x18, 700), thread(0x10, 700), thread(0x18, 700)];
    assert_eq!(
        trace_threads(&twice, 0x18, 700),
        [(0x18, true), (0x18, true), (0x10, false)]
    );
}

/// `walk_stack()`'s choice: the line with the symbol when the lookup
/// succeeded and the symbol is not an export, else the line with the
/// address. The flags are read only after a successful lookup (the C's NULL
/// is `None`, which reading would panic on).
fn symbol_line(success: bool, flags: Option<u32>) -> bool {
    success && names_symbol(flags.expect("the flags read after a failed lookup"))
}

#[test]
fn steps_frame_line() {
    assert_eq!(SYMFLAG_EXPORT, 0x200);

    assert!(names_symbol(0));
    assert!(!names_symbol(0x200));
    assert!(!names_symbol(0x201));
    assert!(!names_symbol(0x1200));
    assert!(!names_symbol(u32::MAX));
    // every other flag (SYMFLAG_FUNCTION 0x800, SYMFLAG_FORWARDER 0x400,
    // ...) still names the symbol
    assert!(names_symbol(0x800));
    assert!(names_symbol(0x400));
    assert!(names_symbol(0x100));
    assert!(names_symbol(!0x200));

    assert!(symbol_line(true, Some(0x800)));
    assert!(!symbol_line(true, Some(0x200)));
    // a failed lookup (or no symbol buffer) never reads the flags
    assert!(!symbol_line(false, None));
}

#[test]
fn steps_masks_32bit() {
    // 32-bit addresses keep every bit
    assert_eq!(mask_address(0x0019_FF3C), 0x0019_FF3C);
    assert_eq!(mask_address(0xFFFF_FFFF), 0xFFFF_FFFF);
    assert_eq!(mask_param(0xFFFF_FFFF), 0xFFFF_FFFF);
    assert_eq!(mask_param(0), 0);

    // above 2^32: parameters keep 32 bits
    assert_eq!(mask_param(0x1234_5678_9ABC_DEF0), 0x9ABC_DEF0);
    assert_eq!(mask_param(0x1_0000_0000), 0);
    assert_eq!(mask_param(u64::MAX), 0xFFFF_FFFF);

    // characterized, not endorsed: the stack and PC keep 36 bits, nine Fs,
    // where the parameters keep 32 (no effect on 32-bit addresses)
    assert_eq!(mask_address(0x1234_5678_9ABC_DEF0), 0x8_9ABC_DEF0);
    assert_eq!(mask_address(0x1_0000_0000), 0x1_0000_0000);
    assert_eq!(mask_address(u64::MAX), 0xF_FFFF_FFFF);
    assert_eq!(mask_address(0x10_0000_0000), 0);
}

#[test]
fn steps_name_prefix() {
    assert!(!name_prefix(0));
    assert!(name_prefix(1));
    assert!(name_prefix(255));
    assert!(name_prefix(usize::MAX));
}

#[test]
fn steps_handler_passes_on() {
    // a debugger: passed on, and the flag is never read
    assert!(pass_on(true, || panic!(
        "the re-entry flag read with a debugger"
    )));
    assert!(pass_on(true, || true));
    assert!(pass_on(true, || false));

    // re-entered: passed on
    assert!(pass_on(false, || true));

    // neither: the report runs
    assert!(!pass_on(false, || false));
}

/// A base near an address, so the edges are hit often.
fn module_strategy() -> impl Strategy<Value = (u64, u64, u32)> {
    (any::<u64>(), any::<u32>(), 0u8..6, any::<u64>()).prop_map(|(addr, size, edge, base)| {
        let s = u64::from(size);
        let base = match edge {
            0 => addr,
            1 => addr.wrapping_add(1),
            2 => addr.wrapping_sub(s),
            3 => addr.wrapping_sub(s).wrapping_add(1),
            4 => u64::MAX - (base % 0x1_0000_0000),
            _ => base,
        };
        (addr, base, size)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// The module rule over any address and module, against the C's
    /// expression: `addr >= base && addr < base + size` in `uint64_t`.
    #[test]
    fn edge_contains_any_module((addr, base, size) in module_strategy()) {
        let end = base.wrapping_add(u64::from(size));
        prop_assert_eq!(contains(addr, base, size), addr >= base && addr < end);
    }

    /// The thread rules over any ids and pass, against the C's
    /// comparisons.
    #[test]
    fn edge_thread_rules_any_thread(
        id in 0u32..4, current in 0u32..4, owner in any::<u32>(), process in any::<u32>(),
        first in any::<bool>(), crashed in any::<bool>(),
    ) {
        prop_assert_eq!(is_crashed(id, current), id == current);
        prop_assert_eq!(is_crashed(owner, process), owner == process);
        prop_assert_eq!(in_pass(first, crashed), first == crashed);
        prop_assert_eq!(other_process(owner, process), owner != process);
        prop_assert_eq!(other_process(id, current), id != current);
    }

    /// The frame line, the masks and the name prefix over any value.
    #[test]
    fn edge_frame_any_value(flags in any::<u32>(), value in any::<u64>(), len in any::<usize>()) {
        prop_assert_eq!(names_symbol(flags), flags & 0x200 == 0);
        prop_assert_eq!(mask_address(value), value & 0xF_FFFF_FFFF);
        prop_assert_eq!(mask_param(value), value & 0xFFFF_FFFF);
        prop_assert_eq!(name_prefix(len), len != 0);
    }

    /// The filter over both inputs: the flag is read exactly when no
    /// debugger is present.
    #[test]
    fn edge_pass_on_any_state(debugger in any::<bool>(), inside in any::<bool>()) {
        let mut reads = 0;
        let passed = pass_on(debugger, || {
            reads += 1;
            inside
        });
        prop_assert_eq!(passed, debugger || inside);
        prop_assert_eq!(reads, usize::from(!debugger));
    }
}

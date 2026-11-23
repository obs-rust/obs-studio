//! Tier 3: the original extracted `obs_reset_source_uuids()` versus the shim
//! on one recording fixture core (oracle/reset_source_uuids.c), through the
//! real-named adapters. The fixture lists real `struct obs_source` entries
//! in the core's real uthash uuid table in the case's order, and records
//! every lock and unlock, every uuid freed (with its source), every uuid
//! generated (numbered the same on both sides), every other free (the
//! tables' own), each with a mark when the sources mutex is not held; then
//! the final table in next-link order with each entry's new uuid, its
//! count and tail, and the lookups by each new uuid (the same source) and
//! old uuid (none). Cases: the C test's five sources, every table size up to
//! the pool, reordered tables, one and two resets, and proptest tables.
//!
//! The working and new table heads are the walk's locals and the stored
//! head is the old head's pointer (the first entry stays first), so the
//! order of the head store is checked by the Tier 1 mirrors, not here.
//! Excluded: an uninitialized core and another thread's table change,
//! outside the original's contract. No intentional differences. Mutants
//! show the probe catches each broken step.
use core::ffi::{CStr, c_char, c_void};
use core::ptr;
use obs_core::ffi::reset_source_uuids::obs_reset_source_uuids;
use proptest::prelude::*;

const POOL: usize = 8;

type Reset = unsafe extern "C" fn();

#[repr(C)]
struct Case {
    order: *const u8,
    count: usize,
    resets: u8,
}

unsafe extern "C" {
    fn oracle_reset_source_uuids_probe(candidate: Reset, case: *const Case) -> u32;
    fn oracle_reset_source_uuids_original(
        case: *const Case,
        trace: *mut c_char,
        size: usize,
    ) -> bool;
    fn oracle_reset_source_uuids_adapters() -> bool;
}

fn case(order: &[u8], resets: u8) -> Case {
    Case {
        order: order.as_ptr(),
        count: order.len(),
        resets,
    }
}

fn probe_with(candidate: Reset, order: &[u8], resets: u8) -> u32 {
    let case = case(order, resets);
    // SAFETY: the oracle owns the fixture core every adapter reaches; the
    // order outlives the call.
    unsafe { oracle_reset_source_uuids_probe(candidate, &case) }
}

fn probe(order: &[u8], resets: u8) -> u32 {
    probe_with(obs_reset_source_uuids, order, resets)
}

/// The original's trace.
fn original(order: &[u8], resets: u8) -> String {
    let case = case(order, resets);
    let mut trace = [0 as c_char; 2048];
    // SAFETY: the buffer is writable for its size; the oracle writes
    // NUL-terminated text.
    let released =
        unsafe { oracle_reset_source_uuids_original(&case, trace.as_mut_ptr(), trace.len()) };
    assert!(released, "the original left the mutex held");
    // SAFETY: NUL-terminated by the oracle.
    unsafe { CStr::from_ptr(trace.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned()
}

/// The C test's populate(): five sources in creation order.
const POPULATED: [u8; 5] = [0, 1, 2, 3, 4];

#[test]
fn real_adapters_read_and_write_the_real_fields() {
    // SAFETY: the oracle arms and frees its own fixture.
    assert!(unsafe { oracle_reset_source_uuids_adapters() });
}

#[test]
fn original_c_is_the_characterized_walk() {
    // Each old uuid freed, then a new one generated, in table order, under
    // the lock; the old table's own blocks freed with its last entry; the
    // order kept and every lookup by the new uuid.
    assert_eq!(
        original(&[2, 0, 1], 1),
        "Lfcg1fag2TTfbg3U|c000001a000002b000003#3=yoyoyo"
    );
    // An empty table: the lock and nothing else.
    assert_eq!(original(&[], 1), "LU|#0");
    // A second reset frees the uuids the first one generated.
    assert_eq!(original(&[1], 2), "LTTfbg1U/LTTfbg2U|b000002#1=yo");
}

#[test]
fn characterized_edges() {
    let mut tables: Vec<Vec<u8>> = vec![
        vec![],
        POPULATED.to_vec(),
        vec![7],
        vec![7, 0],
        vec![3, 1, 4, 0, 6, 2, 5, 7],
    ];
    for count in 1..=POOL {
        let order: Vec<u8> = (0..count).map(|i| i as u8).collect();
        tables.push(order.iter().rev().copied().collect());
        tables.push(order);
    }
    for table in &tables {
        for resets in [1, 2] {
            assert_eq!(probe(table, resets), 0, "{table:?} x{resets}");
        }
    }
}

// Mutants: a walk that breaks one step must be caught.

unsafe extern "C" {
    fn obs_reset_source_uuids_lock();
    fn obs_reset_source_uuids_unlock();
    fn obs_reset_source_uuids_head() -> *mut c_void;
    fn obs_reset_source_uuids_next(ctx: *const c_void) -> *mut c_void;
    fn obs_reset_source_uuids_delete(table: *mut *mut c_void, ctx: *mut c_void);
    fn obs_reset_source_uuids_free_uuid(ctx: *mut c_void);
    fn obs_reset_source_uuids_new_uuid(ctx: *mut c_void);
    fn obs_reset_source_uuids_add(table: *mut *mut c_void, ctx: *mut c_void);
    fn obs_reset_source_uuids_store(table: *mut c_void);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mutant {
    /// Each next link read after the entry is re-added (it is then none).
    NextAfterAdd,
    /// The mutex released and taken again around each entry.
    LockPerEntry,
    /// The old uuid freed before the entry is unlinked.
    FreeBeforeDelete,
    /// No lock at all.
    Unlocked,
}

/// The walk, broken as `mutant` says.
///
/// # Safety
/// The fixture's adapters, on this thread's fixture.
unsafe fn walk(mutant: Mutant) {
    // SAFETY: as the shim, on the fixture core.
    unsafe {
        let locked = mutant != Mutant::Unlocked;
        if locked {
            obs_reset_source_uuids_lock();
        }
        let mut ht = obs_reset_source_uuids_head();
        let mut new_ht: *mut c_void = ptr::null_mut();
        let mut ctx = ht;
        while !ctx.is_null() {
            let next = obs_reset_source_uuids_next(ctx);
            if mutant == Mutant::FreeBeforeDelete {
                obs_reset_source_uuids_free_uuid(ctx);
                obs_reset_source_uuids_delete(&mut ht, ctx);
            } else {
                obs_reset_source_uuids_delete(&mut ht, ctx);
                obs_reset_source_uuids_free_uuid(ctx);
            }
            obs_reset_source_uuids_new_uuid(ctx);
            obs_reset_source_uuids_add(&mut new_ht, ctx);
            if mutant == Mutant::LockPerEntry {
                obs_reset_source_uuids_unlock();
                obs_reset_source_uuids_lock();
            }
            ctx = if mutant == Mutant::NextAfterAdd {
                obs_reset_source_uuids_next(ctx)
            } else {
                next
            };
        }
        obs_reset_source_uuids_store(new_ht);
        if locked {
            obs_reset_source_uuids_unlock();
        }
    }
}

unsafe extern "C" fn next_after_add() {
    // SAFETY: on the fixture.
    unsafe { walk(Mutant::NextAfterAdd) }
}

unsafe extern "C" fn lock_per_entry() {
    // SAFETY: on the fixture.
    unsafe { walk(Mutant::LockPerEntry) }
}

unsafe extern "C" fn free_before_delete() {
    // SAFETY: on the fixture.
    unsafe { walk(Mutant::FreeBeforeDelete) }
}

unsafe extern "C" fn unlocked() {
    // SAFETY: on the fixture.
    unsafe { walk(Mutant::Unlocked) }
}

#[test]
fn mutants_are_caught() {
    for mutant in [next_after_add, lock_per_entry, free_before_delete, unlocked] {
        assert_ne!(probe_with(mutant, &POPULATED, 1), 0);
    }
    // The shim itself matches on the same case.
    assert_eq!(probe(&POPULATED, 1), 0);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_table_matches_original_c(
        order in Just((0..POOL as u8).collect::<Vec<u8>>()).prop_shuffle(),
        count in 0..=POOL,
        resets in 1..=2_u8,
    ) {
        prop_assert_eq!(probe(&order[..count], resets), 0);
    }
}

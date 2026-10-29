//! Tier 3 parity: the shims against the original
//! `libobs/obs-nix-x11-keys.c.h` (oracle/nix_x11_keys.c).
//!
//! `test_nix_x11_keys_steps` compiles the C into itself, so it never sees
//! the Rust swap (a variant linked against `libobs_rust-static` does not
//! link on its own: see `test/cmocka/tests.d/test_nix_x11_keys_steps.cmake`).
//! This file is what compares the two.
//!
//! Both sides get the same tables, the same keyboard map and the same
//! keycode lists, laid out as the C's `struct keycode_list` darrays. Each
//! case runs `fill_keycodes()`'s walk over every row of the map on both
//! sides (the row result, the key written, and the super keycode stores and
//! list pushes the caller makes from them), then compares, on the lists the
//! C side built:
//! - the base keysym lookup of every symbol in the map and of the right
//!   modifiers, the super keysyms and 0;
//! - the keycode lookup of every keycode 0 to 255, also on lists that
//!   share keycodes;
//! - the pressed decision of every key, with the bitmap and without a
//!   reply;
//! - the virtual key search of every symbol in the map, 0, a symbol not in
//!   it, and the map read with `syms_per_code` -1 to 8 and a partial last
//!   row;
//! - the virtual key of every key in the table, and how often each side
//!   asks its getter for the table (never for Meta).
//!
//! Exhaustive over the base table's keys (the virtual key and base keysym
//! of every key, with distinct, shared and zero keysyms); proptests over
//! random keyboard maps (1 to 7 symbols per keycode, keycodes 8 to 255, and
//! other minimum keycodes) and every `int` key.
//!
//! Intentional difference, excluded here and checked on the Rust side only:
//! the virtual key of a key outside `0 .. OBS_KEY_LAST_VALUE - 1`. C indexes
//! its table with it; Rust gives 0 (`OBS_KEY_META` is inside the table, and
//! gives `XK_Super_L` on both sides). Null tables, maps and lists, which C
//! dereferences, are not passed. Nothing else is excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::cell::Cell;
use std::ffi::{c_int, c_void};
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::nix_x11_keys::{
    LAYOUT_NAMES, VALUE_NAMES, oracle_obs_nix_x11_keys_from_base_keysym,
    oracle_obs_nix_x11_keys_from_keycode, oracle_obs_nix_x11_keys_from_virtual_key,
    oracle_obs_nix_x11_keys_pressed, oracle_obs_nix_x11_keys_row,
    oracle_obs_nix_x11_keys_to_virtual_key, oracle_x11_keys_layout, oracle_x11_keys_value,
};
use obs_hotkey::ffi::nix_x11_keys::{
    OBS_NIX_X11_KEYS_ROW_KEY, OBS_NIX_X11_KEYS_ROW_NONE, OBS_NIX_X11_KEYS_ROW_SUPER_L,
    OBS_NIX_X11_KEYS_ROW_SUPER_R, keycode_list, obs_nix_x11_keys_from_base_keysym,
    obs_nix_x11_keys_from_keycode, obs_nix_x11_keys_from_virtual_key, obs_nix_x11_keys_pressed,
    obs_nix_x11_keys_row, obs_nix_x11_keys_to_virtual_key,
};
use obs_hotkey::name_map::{KEY_NONE, OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_META, OBS_KEY_SHIFT};
use obs_hotkey::nix_x11_keys::{
    ALT_R, CONTROL_R, KEYMAP_LEN, OBS_KEY_LAST_VALUE, SHIFT_R, SUPER_L, SUPER_R,
};
use obs_util::ffi::darray::darray;
use proptest::prelude::*;

const SENTINEL: c_int = -7;

/// Keycode lists as both sides read them: one darray per key over buffers
/// this struct owns.
struct Lists {
    bufs: Vec<Vec<u8>>,
    raw: Vec<darray>,
}

impl Lists {
    fn new(bufs: Vec<Vec<u8>>) -> Self {
        assert_eq!(bufs.len(), OBS_KEY_LAST_VALUE);
        let mut lists = Self {
            bufs,
            raw: Vec::new(),
        };
        lists.relink();
        lists
    }

    fn empty() -> Self {
        Self::new(vec![Vec::new(); OBS_KEY_LAST_VALUE])
    }

    /// Points each darray at its buffer again (after a push).
    fn relink(&mut self) {
        self.raw = self
            .bufs
            .iter_mut()
            .map(|b| darray {
                array: if b.is_empty() {
                    std::ptr::null_mut()
                } else {
                    b.as_mut_ptr().cast()
                },
                num: b.len(),
                capacity: b.capacity(),
            })
            .collect();
    }

    fn push(&mut self, key: c_int, code: u8) {
        self.bufs[usize::try_from(key).unwrap()].push(code);
        self.relink();
    }

    fn rust(&self) -> *const keycode_list {
        self.raw.as_ptr().cast()
    }

    fn c(&self) -> *const c_void {
        self.raw.as_ptr().cast()
    }

    fn rust_at(&self, key: usize) -> *const keycode_list {
        self.raw[key..].as_ptr().cast()
    }

    fn c_at(&self, key: usize) -> *const c_void {
        self.raw[key..].as_ptr().cast()
    }
}

thread_local! {
    /// The base table [`table_getter`] returns, and how often it was asked.
    static TABLE: Cell<(*const u32, u32)> = const { Cell::new((std::ptr::null(), 0)) };
}

/// `obs_nix_x11_keys_table_fn` for both sides: the thread's table, counted.
unsafe extern "C" fn table_getter() -> *const u32 {
    TABLE.with(|t| {
        let (table, asks) = t.get();
        t.set((table, asks + 1));
        table
    })
}

/// The virtual key of `key` through `call`, with `table` behind the getter:
/// the result and how often the getter was called.
fn with_table(table: &[u32], call: impl FnOnce() -> c_int) -> (c_int, u32) {
    assert_eq!(table.len(), OBS_KEY_LAST_VALUE);
    TABLE.with(|t| t.set((table.as_ptr(), 0)));
    let result = call();
    let asks = TABLE.with(|t| t.replace((std::ptr::null(), 0)).1);
    (result, asks)
}

/// The shim's virtual key of `key`, and the getter calls.
///
/// # Safety
///
/// `table` has `OBS_KEY_LAST_VALUE` keysyms.
unsafe fn rust_to_virtual(key: c_int, table: &[u32]) -> (c_int, u32) {
    // SAFETY: the getter returns the table.
    with_table(table, || unsafe {
        obs_nix_x11_keys_to_virtual_key(key, Some(table_getter))
    })
}

/// The C's virtual key of `key`, and the getter calls.
///
/// # Safety
///
/// As [`rust_to_virtual`]; C reads past the table for a key outside it.
unsafe fn c_to_virtual(key: c_int, table: &[u32]) -> (c_int, u32) {
    // SAFETY: the getter returns the table.
    with_table(table, || unsafe {
        oracle_obs_nix_x11_keys_to_virtual_key(key, Some(table_getter))
    })
}

/// One side's `fill_keycodes()` walk: the super keycodes and the lists.
#[derive(Debug, PartialEq, Eq)]
struct Filled {
    super_l: u8,
    super_r: u8,
    lists: Vec<Vec<u8>>,
}

type RowFn = unsafe fn(*const u32, c_int, *const u32, *mut c_int) -> c_int;

unsafe fn rust_row(row: *const u32, per: c_int, base: *const u32, key: *mut c_int) -> c_int {
    // SAFETY: forwarded.
    unsafe { obs_nix_x11_keys_row(row, per, base, key) }
}

unsafe fn c_row(row: *const u32, per: c_int, base: *const u32, key: *mut c_int) -> c_int {
    // SAFETY: forwarded.
    unsafe { oracle_obs_nix_x11_keys_row(row, per, base, key) }
}

/// `fill_keycodes()`'s loop over every keycode from `min` on, with `row`
/// for the decision and the caller's stores and `add_key()` pushes.
fn fill(row: RowFn, table: &[u32], map: &[u32], per: c_int, min: u8, rows: usize) -> Filled {
    let mut lists = Lists::empty();
    let (mut super_l, mut super_r) = (0, 0);
    let per_len = usize::try_from(per).unwrap();
    for i in 0..rows {
        let code = (usize::from(min) + i) as u8;
        let mut key = SENTINEL;
        // SAFETY: the row has `per` symbols, the table OBS_KEY_LAST_VALUE.
        let what = unsafe { row(map[i * per_len..].as_ptr(), per, table.as_ptr(), &mut key) };
        match what {
            OBS_NIX_X11_KEYS_ROW_SUPER_L => super_l = code,
            OBS_NIX_X11_KEYS_ROW_SUPER_R => super_r = code,
            OBS_NIX_X11_KEYS_ROW_KEY => lists.push(key, code),
            OBS_NIX_X11_KEYS_ROW_NONE => assert_eq!(key, SENTINEL, "key written without a key"),
            other => panic!("row result {other}"),
        }
    }
    Filled {
        super_l,
        super_r,
        lists: lists.bufs,
    }
}

/// One random or made-up keyboard and its expected answers on both sides.
#[derive(Debug, Clone)]
struct Case {
    table: Vec<u32>,
    per: c_int,
    min: u8,
    rows: usize,
    map: Vec<u32>,
    bitmap: [u8; KEYMAP_LEN],
    extra: Vec<u32>,
}

fn sym(s: u32) -> c_int {
    s as c_int
}

fn check_case(case: &Case) {
    let Case {
        table,
        per,
        min,
        rows,
        map,
        ..
    } = case;
    assert_eq!(table.len(), OBS_KEY_LAST_VALUE);

    // The walk at init: every row, the stores and the pushes.
    let rust = fill(rust_row, table, map, *per, *min, *rows);
    let c = fill(c_row, table, map, *per, *min, *rows);
    assert_eq!(rust, c, "fill_keycodes walk");
    check_rows(case);
    check_lookups(case, &Lists::new(c.lists), (rust.super_l, rust.super_r));
}

/// Each row of the map with every length up to `per`.
fn check_rows(case: &Case) {
    let Case {
        table,
        per,
        rows,
        map,
        ..
    } = case;
    // Each row also with every shorter length (a zero ends it early or not).
    for i in 0..*rows {
        let start = i * usize::try_from(*per).unwrap();
        for len in -1..=*per {
            let (mut rk, mut ck) = (SENTINEL, SENTINEL);
            // SAFETY: at most `per` symbols of the row; the table is full.
            let (r, cr) = unsafe {
                (
                    obs_nix_x11_keys_row(map[start..].as_ptr(), len, table.as_ptr(), &mut rk),
                    oracle_obs_nix_x11_keys_row(
                        map[start..].as_ptr(),
                        len,
                        table.as_ptr(),
                        &mut ck,
                    ),
                )
            };
            assert_eq!((r, rk), (cr, ck), "row {i} of length {len}");
        }
    }
}

/// The lookups on the lists the walk built; `supers` are the super
/// keycodes it found (the same on both sides).
fn check_lookups(case: &Case, lists: &Lists, supers: (u8, u8)) {
    let Case {
        table,
        per,
        min,
        map,
        bitmap,
        extra,
        ..
    } = case;
    let (super_l, super_r) = supers;

    // The base keysym lookup.
    let specials = [0, SHIFT_R, CONTROL_R, ALT_R, SUPER_L, SUPER_R];
    for &s in map.iter().chain(extra).chain(&specials) {
        // SAFETY: the table has OBS_KEY_LAST_VALUE keysyms.
        let (r, cr) = unsafe {
            (
                obs_nix_x11_keys_from_base_keysym(table.as_ptr(), s),
                oracle_obs_nix_x11_keys_from_base_keysym(table.as_ptr(), s),
            )
        };
        assert_eq!(r, cr, "base keysym {s:#x}");
    }

    // The keycode lookup, on these lists and on lists that share keycodes
    // (the walk gives each keycode to one key only; the lookup's first
    // match matters only when they share).
    let shared = Lists::new(
        (0..OBS_KEY_LAST_VALUE)
            .map(|k| {
                let k = u8::try_from(k % 256).unwrap();
                vec![k.wrapping_mul(7), k.wrapping_mul(13).wrapping_add(*min)]
            })
            .collect(),
    );
    for set in [lists, &shared] {
        for code in 0..=255u8 {
            // SAFETY: OBS_KEY_LAST_VALUE lists.
            let (r, cr) = unsafe {
                (
                    obs_nix_x11_keys_from_keycode(set.rust(), code),
                    oracle_obs_nix_x11_keys_from_keycode(set.c(), code),
                )
            };
            assert_eq!(r, cr, "keycode {code}");
        }
    }

    // The pressed decision of every key, with and without a reply.
    for key in 0..OBS_KEY_LAST_VALUE {
        let k = c_int::try_from(key).unwrap();
        for keys in [bitmap.as_ptr(), std::ptr::null()] {
            // SAFETY: a 32-byte bitmap or null; the key's own list.
            let (r, cr) = unsafe {
                (
                    obs_nix_x11_keys_pressed(keys, k, lists.rust_at(key), &super_l, &super_r),
                    oracle_obs_nix_x11_keys_pressed(keys, k, lists.c_at(key), &super_l, &super_r),
                )
            };
            assert_eq!(r, cr, "pressed {key}, reply {}", !keys.is_null());
        }
    }

    // The virtual key search, the map read with other row sizes too.
    let num = c_int::try_from(map.len()).unwrap();
    for &s in map.iter().chain(extra).chain(&specials) {
        for p in [-1, 0, 1, *per, *per + 1, 8] {
            // SAFETY: `num` keysyms; OBS_KEY_LAST_VALUE lists.
            let (r, cr) = unsafe {
                (
                    obs_nix_x11_keys_from_virtual_key(
                        sym(s),
                        map.as_ptr(),
                        p,
                        &num,
                        min,
                        lists.rust(),
                    ),
                    oracle_obs_nix_x11_keys_from_virtual_key(
                        sym(s),
                        map.as_ptr(),
                        p,
                        &num,
                        min,
                        lists.c(),
                    ),
                )
            };
            assert_eq!(r, cr, "virtual key {s:#x}, {p} per keycode");
        }
    }

    // The virtual key of every key.
    for key in 0..OBS_KEY_LAST_VALUE {
        let k = c_int::try_from(key).unwrap();
        // SAFETY: the table has OBS_KEY_LAST_VALUE keysyms.
        let (r, cr) = unsafe { (rust_to_virtual(k, table), c_to_virtual(k, table)) };
        assert_eq!(r, cr, "to virtual key {key} (result, table asks)");
    }
}

#[test]
fn values_and_layout_match_the_c() {
    let rust_values = [
        i64::try_from(OBS_KEY_LAST_VALUE).unwrap(),
        i64::from(KEY_NONE),
        i64::from(OBS_KEY_SHIFT),
        i64::from(OBS_KEY_CONTROL),
        i64::from(OBS_KEY_ALT),
        i64::from(OBS_KEY_META),
        i64::from(SHIFT_R),
        i64::from(CONTROL_R),
        i64::from(ALT_R),
        i64::from(SUPER_L),
        i64::from(SUPER_R),
        i64::from(OBS_NIX_X11_KEYS_ROW_NONE),
        i64::from(OBS_NIX_X11_KEYS_ROW_SUPER_L),
        i64::from(OBS_NIX_X11_KEYS_ROW_SUPER_R),
        i64::from(OBS_NIX_X11_KEYS_ROW_KEY),
    ];
    for (i, (name, value)) in VALUE_NAMES.iter().zip(rust_values).enumerate() {
        // SAFETY: a plain getter.
        assert_eq!(unsafe { oracle_x11_keys_value(i) }, value, "{name}");
    }

    let rust_layout = [
        size_of::<keycode_list>(),
        align_of::<keycode_list>(),
        offset_of!(darray, array),
        offset_of!(darray, num),
        offset_of!(darray, capacity),
    ];
    for (i, (name, value)) in LAYOUT_NAMES.iter().zip(rust_layout).enumerate() {
        // SAFETY: a plain getter.
        assert_eq!(unsafe { oracle_x11_keys_layout(i) }, value, "{name}");
    }
    assert_eq!(size_of::<keycode_list>(), size_of::<darray>());
}

/// Every key of base tables with distinct, shared and zero keysyms: its
/// virtual key, and the key of its keysym, on both sides.
#[test]
fn every_base_table_key() {
    let distinct: Vec<u32> = (0..OBS_KEY_LAST_VALUE)
        .map(|i| 0x1000 + u32::try_from(i).unwrap())
        .collect();
    let shared: Vec<u32> = (0..OBS_KEY_LAST_VALUE)
        .map(|i| 0x20 + u32::try_from(i % 7).unwrap())
        .collect();
    let zero = vec![0u32; OBS_KEY_LAST_VALUE];
    let mut modifiers = distinct.clone();
    modifiers[1] = SHIFT_R;
    modifiers[2] = SUPER_L;
    modifiers[3] = u32::MAX;

    for table in [&distinct, &shared, &zero, &modifiers] {
        for key in 0..OBS_KEY_LAST_VALUE {
            let k = c_int::try_from(key).unwrap();
            // SAFETY: the tables have OBS_KEY_LAST_VALUE keysyms.
            unsafe {
                assert_eq!(
                    rust_to_virtual(k, table),
                    c_to_virtual(k, table),
                    "to virtual key {key}"
                );
                assert_eq!(
                    obs_nix_x11_keys_from_base_keysym(table.as_ptr(), table[key]),
                    oracle_obs_nix_x11_keys_from_base_keysym(table.as_ptr(), table[key]),
                    "base keysym of key {key}"
                );
            }
        }
    }
}

/// The C test's layout and a US-like map, through every comparison.
#[test]
fn fixed_keyboards() {
    let mut table = vec![0u32; OBS_KEY_LAST_VALUE];
    for (i, t) in table.iter_mut().enumerate().skip(1).take(120) {
        *t = 0x20 + u32::try_from(i).unwrap();
    }
    table[usize::try_from(OBS_KEY_SHIFT).unwrap()] = 0xffe1;
    let mut map = Vec::new();
    for code in 8..=255u32 {
        let base = 0x20 + (code % 100);
        map.extend([base, base + 1, if code % 5 == 0 { 0 } else { base + 2 }, 0]);
    }
    map[4 * 50] = SUPER_L;
    map[4 * 51 + 1] = SUPER_R;
    map[4 * 52] = SHIFT_R;
    map[4 * 53 + 2] = ALT_R;
    let mut bitmap = [0u8; KEYMAP_LEN];
    for (i, b) in bitmap.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(37);
    }
    check_case(&Case {
        table,
        per: 4,
        min: 8,
        rows: 248,
        map,
        bitmap,
        extra: vec![0x10ffff, 0x7f],
    });
}

/// The intentional difference: keys outside the table give 0 in Rust (C
/// reads past its table, so it is not called).
#[test]
fn keys_outside_the_table_give_zero() {
    let table = vec![0x61u32; OBS_KEY_LAST_VALUE];
    let last = c_int::try_from(OBS_KEY_LAST_VALUE).unwrap();
    for key in [c_int::MIN, -1000, -1, last, last + 1, c_int::MAX] {
        // SAFETY: the table has OBS_KEY_LAST_VALUE keysyms.
        assert_eq!(unsafe { rust_to_virtual(key, &table) }.0, 0, "{key}");
    }
}

fn keysym() -> impl Strategy<Value = u32> {
    prop_oneof![
        4 => Just(0u32),
        2 => prop::sample::select(vec![SHIFT_R, CONTROL_R, ALT_R, SUPER_L, SUPER_R, 0xffe1]),
        8 => 0x20u32..0x60,
        1 => any::<u32>(),
    ]
}

fn case() -> impl Strategy<Value = Case> {
    (
        1i32..=7,
        prop_oneof![4 => Just(8u8), 1 => any::<u8>()],
        any::<u8>(),
    )
        .prop_flat_map(|(per, min, max)| {
            let max = max.max(min);
            let rows = usize::from(max - min) + 1;
            let len = rows * usize::try_from(per).unwrap();
            (
                prop::collection::vec(keysym(), OBS_KEY_LAST_VALUE),
                Just(per),
                Just(min),
                Just(rows),
                prop::collection::vec(keysym(), len..=len + 3),
                prop::array::uniform32(any::<u8>()),
                prop::collection::vec(keysym(), 0..4),
            )
        })
        .prop_map(|(table, per, min, rows, map, bitmap, extra)| Case {
            table,
            per,
            min,
            rows,
            map,
            bitmap,
            extra,
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    #[test]
    fn random_keyboards(case in case()) {
        check_case(&case);
    }

    /// Every `int` key: the C's answer inside the table, 0 outside.
    #[test]
    fn every_int_key(table in prop::collection::vec(keysym(), OBS_KEY_LAST_VALUE), key in any::<c_int>()) {
        // SAFETY: the table has OBS_KEY_LAST_VALUE keysyms.
        let r = unsafe { rust_to_virtual(key, &table) };
        match usize::try_from(key) {
            Ok(i) if i < OBS_KEY_LAST_VALUE => {
                // SAFETY: inside the table.
                let cr = unsafe { c_to_virtual(key, &table) };
                prop_assert_eq!(r, cr);
            }
            // The table is asked for, as C would; its answer is not read.
            _ => prop_assert_eq!(r, (0, 1)),
        }
    }
}

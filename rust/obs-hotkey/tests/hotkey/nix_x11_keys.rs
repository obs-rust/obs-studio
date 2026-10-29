//! Tier 1: the safe core of `libobs/obs-nix-x11-keys.c.h`.
//!
//! The first eleven tests port `test/cmocka/test_nix_x11_keys_steps.c` one to
//! one, with the same names, inputs and expectations, in the same order.
//! Each starts from the C test's `setup()`: [`Fixture::new`].
//!
//! The cases after those pin the edges of each function, and the proptests
//! check each one against a model written from the C.

use obs_hotkey::name_map::{KEY_NONE, key_from_name};
use obs_hotkey::nix_x11_keys::{
    self as xk, ALT_R, CONTROL_R, KEYMAP_LEN, OBS_KEY_LAST_VALUE, Row, SHIFT_R, SUPER_L, SUPER_R,
};
use proptest::prelude::*;

const XK_A_LOWER: u32 = 0x61;
const XK_B_LOWER: u32 = 0x62;
const XK_A_UPPER: u32 = 0x41;
const XK_1: u32 = 0x31;
const XK_EXCLAM: u32 = 0x21;
const XK_BRACKETLEFT: u32 = 0x5b;
const XK_8: u32 = 0x38;
const XK_ASTERISK: u32 = 0x2a;
const XK_SHIFT_L: u32 = 0xffe1;
const XK_UNKNOWN: u32 = 0x10ffff;

/// The code of key `name` (`OBS_KEY_<name>`).
fn k(name: &str) -> i32 {
    let full = std::ffi::CString::new(format!("OBS_KEY_{name}")).unwrap();
    let key = key_from_name(&full);
    assert!(name == "NONE" || key != KEY_NONE, "no key {name}");
    key
}

fn idx(key: i32) -> usize {
    usize::try_from(key).unwrap()
}

/// The C test's static tables after `setup()`.
struct Fixture {
    base: Vec<u32>,
    lists: Vec<Vec<u8>>,
}

impl Fixture {
    fn new() -> Self {
        let mut base = vec![0; OBS_KEY_LAST_VALUE];
        base[idx(k("A"))] = XK_A_LOWER;
        base[idx(k("B"))] = XK_B_LOWER;
        base[idx(k("1"))] = XK_1;
        base[idx(k("8"))] = XK_8;
        base[idx(k("BRACKETLEFT"))] = XK_BRACKETLEFT;
        base[idx(k("SHIFT"))] = XK_SHIFT_L;
        Self {
            base,
            lists: vec![Vec::new(); OBS_KEY_LAST_VALUE],
        }
    }

    fn set_codes(&mut self, key: i32, codes: &[u8]) {
        self.lists[idx(key)] = codes.to_vec();
    }

    fn row(&self, row: &[u32]) -> Row {
        xk::row_key(row, &self.base)
    }

    fn virtual_key(&self, sym: i32, per: i32) -> i32 {
        from_virtual(sym, &MAP, per, MIN_CODE, &self.lists)
    }
}

/// The lookups with every input at hand, as the C test's helpers call
/// them; the core asks for them through closures.
fn any_pressed(
    keys: Option<&[u8; KEYMAP_LEN]>,
    key: i32,
    codes: &[u8],
    super_l: u8,
    super_r: u8,
) -> bool {
    xk::any_pressed(keys, key, || codes, || super_l, || super_r)
}

fn from_virtual<L: AsRef<[u8]>>(sym: i32, keysyms: &[u32], per: i32, min: u8, lists: &[L]) -> i32 {
    xk::from_virtual(sym, per, || keysyms, || min, || lists)
}

fn to_virtual_of(key: i32, table: &[u32]) -> i32 {
    xk::to_virtual(key, || table)
}

/// A closure input that must not be asked for.
fn never<T>(what: &'static str) -> impl FnOnce() -> T {
    move || panic!("{what} was asked for")
}

fn set_down(keys: &mut [u8; KEYMAP_LEN], code: u8) {
    keys[usize::from(code / 8)] |= 1 << (code % 8);
}

/// `row_of()` in the C test: the row's first `per` symbols (none for 0 or
/// less, as the C loop reads none).
fn per(row: &[u32], per: i32) -> &[u32] {
    &row[..usize::try_from(per).unwrap_or(0)]
}

#[test]
fn base_keysym_lookup() {
    let f = Fixture::new();
    let b = &f.base;
    assert_eq!(xk::key_from_base(b, XK_A_LOWER), k("A"));
    assert_eq!(xk::key_from_base(b, XK_SHIFT_L), k("SHIFT"));
    assert_eq!(xk::key_from_base(b, SHIFT_R), k("SHIFT"));
    assert_eq!(xk::key_from_base(b, CONTROL_R), k("CONTROL"));
    assert_eq!(xk::key_from_base(b, ALT_R), k("ALT"));
    assert_eq!(xk::key_from_base(b, XK_UNKNOWN), KEY_NONE);
    assert_eq!(xk::key_from_base(b, SUPER_L), KEY_NONE);
    assert_eq!(xk::key_from_base(b, SUPER_R), KEY_NONE);
    assert_eq!(xk::key_from_base(b, 0), KEY_NONE);
}

#[test]
fn base_keysym_first_wins() {
    let mut f = Fixture::new();
    f.base[idx(k("C"))] = XK_A_LOWER;
    assert_eq!(xk::key_from_base(&f.base, XK_A_LOWER), k("A"));
    f.base[idx(k("ALTGR"))] = ALT_R;
    assert_eq!(xk::key_from_base(&f.base, ALT_R), k("ALTGR"));
    f.base[idx(KEY_NONE)] = XK_B_LOWER;
    assert_eq!(xk::key_from_base(&f.base, XK_B_LOWER), KEY_NONE);
    assert_eq!(xk::key_from_base(&f.base, 0), k("RETURN"));
}

#[test]
fn row_rule() {
    let f = Fixture::new();

    let plain = [XK_A_LOWER, XK_A_UPPER, 0, 0];
    assert_eq!(f.row(per(&plain, 4)), Row::Key(k("A")));

    let first_zero = [0, XK_A_LOWER, SUPER_L, XK_B_LOWER];
    assert_eq!(f.row(per(&first_zero, 4)), Row::None);

    let super_l = [XK_UNKNOWN, SUPER_L, XK_A_LOWER, 0];
    assert_eq!(f.row(per(&super_l, 4)), Row::SuperL);

    let super_r = [SUPER_R, XK_A_LOWER];
    assert_eq!(f.row(per(&super_r, 2)), Row::SuperR);

    let level_two = [XK_EXCLAM, XK_1];
    assert_eq!(f.row(per(&level_two, 2)), Row::Key(k("1")));

    let key_then_super = [XK_B_LOWER, SUPER_R];
    assert_eq!(f.row(per(&key_then_super, 2)), Row::Key(k("B")));
    let super_then_key = [SUPER_L, XK_B_LOWER];
    assert_eq!(f.row(per(&super_then_key, 2)), Row::SuperL);

    let zero_later = [XK_UNKNOWN, 0, XK_A_LOWER];
    assert_eq!(f.row(per(&zero_later, 3)), Row::None);

    let unknown = [XK_UNKNOWN, XK_UNKNOWN + 1, XK_UNKNOWN + 2];
    assert_eq!(f.row(per(&unknown, 3)), Row::None);

    let right_ctrl = [CONTROL_R];
    assert_eq!(f.row(per(&right_ctrl, 1)), Row::Key(k("CONTROL")));

    let short_row = [XK_UNKNOWN, XK_A_LOWER];
    assert_eq!(f.row(per(&short_row, 1)), Row::None);
    assert_eq!(f.row(per(&plain, 0)), Row::None);
    assert_eq!(f.row(per(&plain, -1)), Row::None);
}

#[test]
fn pressed_rule() {
    let mut f = Fixture::new();
    let mut keys = [0u8; KEYMAP_LEN];
    let (a, b, meta) = (k("A"), k("B"), k("META"));

    f.set_codes(a, &[38, 140]);
    let codes = |f: &Fixture, key: i32| f.lists[idx(key)].clone();
    assert!(!any_pressed(Some(&keys), a, &codes(&f, a), 133, 134));

    set_down(&mut keys, 140);
    assert!(any_pressed(Some(&keys), a, &codes(&f, a), 133, 134));
    assert!(!any_pressed(Some(&keys), b, &codes(&f, b), 133, 134));

    assert!(!any_pressed(Some(&keys), meta, &codes(&f, a), 133, 134));
    set_down(&mut keys, 134);
    assert!(any_pressed(Some(&keys), meta, &codes(&f, b), 133, 134));
    keys = [0; KEYMAP_LEN];
    set_down(&mut keys, 133);
    assert!(any_pressed(Some(&keys), meta, &codes(&f, b), 133, 134));

    keys = [0; KEYMAP_LEN];
    set_down(&mut keys, 255);
    assert!(!any_pressed(Some(&keys), meta, &codes(&f, b), 0, 0));
    f.set_codes(b, &[255]);
    assert!(any_pressed(Some(&keys), b, &codes(&f, b), 0, 0));

    assert!(!any_pressed(None, b, &codes(&f, b), 255, 255));
    assert!(!any_pressed(None, meta, &codes(&f, b), 255, 255));
}

#[test]
fn keycode_lookup() {
    let mut f = Fixture::new();
    f.set_codes(k("A"), &[38, 140]);
    f.set_codes(k("C"), &[54, 140]);
    assert_eq!(xk::key_from_keycode(&f.lists, 38), k("A"));
    assert_eq!(xk::key_from_keycode(&f.lists, 54), k("C"));
    assert_eq!(xk::key_from_keycode(&f.lists, 140), k("A"));
    assert_eq!(xk::key_from_keycode(&f.lists, 39), KEY_NONE);
    assert_eq!(xk::key_from_keycode(&f.lists, 0), KEY_NONE);
}

/// The C test's `map`: keycodes 16 to 19, two symbols each.
const MIN_CODE: u8 = 16;
const MAP: [u32; 8] = [
    0,
    0,
    XK_8,
    XK_ASTERISK,
    XK_BRACKETLEFT,
    XK_A_UPPER,
    XK_1,
    XK_BRACKETLEFT,
];

fn sym(s: u32) -> i32 {
    s as i32
}

#[test]
fn from_virtual_columns() {
    let mut f = Fixture::new();
    f.set_codes(k("8"), &[17]);
    f.set_codes(k("BRACKETLEFT"), &[18]);
    f.set_codes(k("1"), &[19]);

    assert_eq!(f.virtual_key(sym(XK_8), 2), k("8"));
    assert_eq!(f.virtual_key(sym(XK_BRACKETLEFT), 2), k("BRACKETLEFT"));
    assert_eq!(f.virtual_key(sym(XK_ASTERISK), 2), k("8"));
    assert_eq!(f.virtual_key(sym(XK_A_UPPER), 2), k("BRACKETLEFT"));
    assert_eq!(f.virtual_key(sym(XK_UNKNOWN), 2), KEY_NONE);

    f.set_codes(k("A"), &[20]);
    assert_eq!(f.virtual_key(sym(XK_BRACKETLEFT), 1), k("A"));
}

#[test]
fn from_virtual_first_hit_only() {
    let mut f = Fixture::new();
    f.set_codes(k("1"), &[19]);
    assert_eq!(f.virtual_key(sym(XK_BRACKETLEFT), 2), KEY_NONE);
    assert_eq!(f.virtual_key(sym(XK_1), 2), k("1"));
}

#[test]
fn from_virtual_gate() {
    let mut f = Fixture::new();
    f.set_codes(k("ESCAPE"), &[16]);
    assert_eq!(f.virtual_key(0, 2), KEY_NONE);
    assert_eq!(f.virtual_key(sym(XK_8), 0), KEY_NONE);
    assert_eq!(f.virtual_key(sym(XK_8), -2), KEY_NONE);
    assert_eq!(
        from_virtual(sym(XK_8), &[], 2, MIN_CODE, &f.lists),
        KEY_NONE
    );
    assert_eq!(f.virtual_key(-1, 2), KEY_NONE);
}

#[test]
fn from_virtual_min_keycode() {
    let mut f = Fixture::new();
    let two_rows = [XK_A_LOWER, XK_B_LOWER];
    f.set_codes(k("A"), &[255]);
    f.set_codes(k("B"), &[0]);
    assert_eq!(
        from_virtual(sym(XK_A_LOWER), &two_rows, 1, 255, &f.lists),
        k("A")
    );
    assert_eq!(
        from_virtual(sym(XK_B_LOWER), &two_rows, 1, 255, &f.lists),
        k("B")
    );
    assert_eq!(
        from_virtual(sym(XK_B_LOWER), &two_rows[..1], 1, 255, &f.lists),
        KEY_NONE
    );
}

#[test]
fn to_virtual() {
    let mut f = Fixture::new();
    let meta = k("META");
    assert_eq!(to_virtual_of(meta, &f.base), sym(SUPER_L));
    f.base[idx(meta)] = SUPER_R;
    assert_eq!(to_virtual_of(meta, &f.base), sym(SUPER_L));
    assert_eq!(to_virtual_of(k("A"), &f.base), sym(XK_A_LOWER));
    assert_eq!(to_virtual_of(k("SHIFT"), &f.base), sym(XK_SHIFT_L));
    assert_eq!(to_virtual_of(KEY_NONE, &f.base), 0);
    assert_eq!(to_virtual_of(k("C"), &f.base), 0);
    let last = i32::try_from(OBS_KEY_LAST_VALUE - 1).unwrap();
    f.base[idx(last)] = 0xffff_fffe;
    assert_eq!(to_virtual_of(last, &f.base), -2);
}

#[test]
fn reads_under_their_guards() {
    let mut f = Fixture::new();
    let mut keys = [0u8; KEYMAP_LEN];
    let (a, meta) = (k("A"), k("META"));

    f.set_codes(a, &[38]);
    set_down(&mut keys, 38);
    let codes = f.lists[idx(a)].clone();
    assert!(xk::any_pressed(
        Some(&keys),
        a,
        || codes,
        never("super_l"),
        never("super_r")
    ));
    set_down(&mut keys, 133);
    assert!(xk::any_pressed(
        Some(&keys),
        meta,
        never::<Vec<u8>>("codes"),
        || 133,
        never("super_r")
    ));

    assert_eq!(
        xk::from_virtual(
            0,
            2,
            never::<&[u32]>("keysyms"),
            never("min_keycode"),
            never::<&[Vec<u8>]>("lists")
        ),
        KEY_NONE
    );
    assert_eq!(
        xk::from_virtual(
            sym(XK_8),
            0,
            never::<&[u32]>("keysyms"),
            never("min_keycode"),
            never::<&[Vec<u8>]>("lists")
        ),
        KEY_NONE
    );
    assert_eq!(
        xk::from_virtual(
            sym(XK_UNKNOWN),
            2,
            || &MAP[..],
            never("min_keycode"),
            never::<&[Vec<u8>]>("lists")
        ),
        KEY_NONE
    );

    let asks = std::cell::Cell::new(0);
    let counted = || {
        asks.set(asks.get() + 1);
        f.base.clone()
    };
    assert_eq!(xk::to_virtual(meta, counted), sym(SUPER_L));
    assert_eq!(asks.get(), 0);
    assert_eq!(xk::to_virtual(a, counted), sym(XK_A_LOWER));
    assert_eq!(asks.get(), 1);
}

// Edge cases beyond the C test.

/// A key outside the table has no keysym: 0, where C reads past the table.
#[test]
fn edge_to_virtual_outside_table() {
    let f = Fixture::new();
    let last = i32::try_from(OBS_KEY_LAST_VALUE).unwrap();
    for key in [-1, i32::MIN, last, last + 1, i32::MAX] {
        assert_eq!(to_virtual_of(key, &f.base), 0, "key {key}");
    }
    assert_eq!(to_virtual_of(k("A"), &[]), 0);
    // Meta is Super_L before any table lookup.
    assert_eq!(to_virtual_of(k("META"), &[]), sym(SUPER_L));
}

/// The lookups read only the tables they are given.
#[test]
fn edge_short_tables() {
    assert_eq!(xk::key_from_base(&[], XK_A_LOWER), KEY_NONE);
    assert_eq!(xk::key_from_base(&[], SHIFT_R), k("SHIFT"));
    assert_eq!(xk::key_from_base(&[7, XK_A_LOWER], XK_A_LOWER), 1);
    assert_eq!(xk::row_key(&[XK_A_LOWER], &[]), Row::None);
    assert_eq!(xk::row_key(&[ALT_R], &[]), Row::Key(k("ALT")));
    let none: [Vec<u8>; 0] = [];
    assert_eq!(xk::key_from_keycode(&none, 38), KEY_NONE);
    assert_eq!(from_virtual(sym(XK_8), &MAP, 2, MIN_CODE, &none), KEY_NONE);
}

/// The map's rows are `keysyms.len() / syms_per_code`: a partial last row
/// is not searched.
#[test]
fn edge_partial_last_row() {
    let mut f = Fixture::new();
    f.set_codes(k("A"), &[10]);
    f.set_codes(k("B"), &[11]);
    let map = [XK_UNKNOWN, XK_UNKNOWN, XK_UNKNOWN, XK_A_LOWER, XK_B_LOWER];
    assert_eq!(
        from_virtual(sym(XK_A_LOWER), &map, 3, 10, &f.lists),
        KEY_NONE
    );
    assert_eq!(from_virtual(sym(XK_A_LOWER), &map, 2, 10, &f.lists), k("B"));
    assert_eq!(
        from_virtual(sym(XK_B_LOWER), &map, 2, 10, &f.lists),
        KEY_NONE
    );
    assert_eq!(
        from_virtual(sym(XK_B_LOWER), &map, 1, 10, &f.lists),
        KEY_NONE
    );
    assert_eq!(
        from_virtual(sym(XK_A_LOWER), &map, 6, 10, &f.lists),
        KEY_NONE
    );
}

/// Every keycode's bit, and only that bit.
#[test]
fn edge_every_keycode_bit() {
    let other = k("A");
    for code in 0..=255u8 {
        let mut keys = [0u8; KEYMAP_LEN];
        set_down(&mut keys, code);
        assert!(any_pressed(Some(&keys), other, &[code], 0, 0));
        assert!(!any_pressed(Some(&keys), other, &[code ^ 1], 0, 0));
        assert!(any_pressed(Some(&keys), k("META"), &[], code, code ^ 1));
        assert!(any_pressed(Some(&keys), k("META"), &[], code ^ 1, code));
        assert!(!any_pressed(
            Some(&keys),
            k("META"),
            &[code],
            code ^ 1,
            code ^ 1
        ));
    }
}

// Models written from the C, for the proptests.

fn model_from_base(table: &[u32], code: u32) -> i32 {
    if let Some(i) = table.iter().position(|&s| s == code) {
        return i32::try_from(i).unwrap();
    }
    match code {
        SHIFT_R => k("SHIFT"),
        CONTROL_R => k("CONTROL"),
        ALT_R => k("ALT"),
        _ => KEY_NONE,
    }
}

fn model_row(row: &[u32], table: &[u32]) -> Row {
    for &s in row {
        if s == 0 {
            break;
        }
        if s == SUPER_L {
            return Row::SuperL;
        }
        if s == SUPER_R {
            return Row::SuperR;
        }
        let key = model_from_base(table, s);
        if key != KEY_NONE {
            return Row::Key(key);
        }
    }
    Row::None
}

fn model_from_keycode(lists: &[Vec<u8>], code: u8) -> i32 {
    lists
        .iter()
        .position(|l| l.contains(&code))
        .map_or(KEY_NONE, |i| i32::try_from(i).unwrap())
}

fn model_from_virtual(sym: i32, keysyms: &[u32], per: i32, min: u8, lists: &[Vec<u8>]) -> i32 {
    if sym == 0 || per <= 0 {
        return KEY_NONE;
    }
    let per = usize::try_from(per).unwrap();
    let rows = keysyms.len() / per;
    for col in 0..per {
        for i in 0..rows {
            if keysyms[i * per + col] == sym as u32 {
                let code = ((i + usize::from(min)) % 256) as u8;
                return model_from_keycode(lists, code);
            }
        }
    }
    KEY_NONE
}

/// Keysyms from a small pool, so that rows and tables share some.
fn keysym() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => Just(0u32),
        2 => prop::sample::select(vec![SHIFT_R, CONTROL_R, ALT_R, SUPER_L, SUPER_R]),
        6 => 0x20u32..0x30,
        1 => any::<u32>(),
    ]
}

fn table() -> impl Strategy<Value = Vec<u32>> {
    prop::collection::vec(keysym(), OBS_KEY_LAST_VALUE)
}

fn lists() -> impl Strategy<Value = Vec<Vec<u8>>> {
    prop::collection::vec(
        prop::collection::vec(prop_oneof![8u8..24, any::<u8>()], 0..3),
        OBS_KEY_LAST_VALUE,
    )
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn prop_key_from_base(table in table(), code in keysym()) {
        prop_assert_eq!(xk::key_from_base(&table, code), model_from_base(&table, code));
    }

    #[test]
    fn prop_row_key(table in table(), row in prop::collection::vec(keysym(), 0..8)) {
        prop_assert_eq!(xk::row_key(&row, &table), model_row(&row, &table));
    }

    #[test]
    fn prop_any_pressed(
        keys in prop::option::of(prop::array::uniform32(any::<u8>())),
        key in -2i32..600,
        codes in prop::collection::vec(any::<u8>(), 0..4),
        super_l in any::<u8>(),
        super_r in any::<u8>(),
    ) {
        let down = |c: u8| keys.is_some_and(|k| k[usize::from(c / 8)] & (1 << (c % 8)) != 0);
        let expect = if key == k("META") {
            down(super_l) || down(super_r)
        } else {
            codes.iter().any(|&c| down(c))
        };
        prop_assert_eq!(any_pressed(keys.as_ref(), key, &codes, super_l, super_r), expect);
    }

    #[test]
    fn prop_key_from_keycode(lists in lists(), code in any::<u8>()) {
        prop_assert_eq!(xk::key_from_keycode(&lists, code), model_from_keycode(&lists, code));
    }

    #[test]
    fn prop_from_virtual(
        lists in lists(),
        per in -1i32..8,
        rows in 0usize..40,
        extra in 0usize..3,
        min in prop_oneof![Just(8u8), any::<u8>()],
        syms in prop::collection::vec(keysym(), 0..400),
        pick in any::<prop::sample::Index>(),
        sym in keysym(),
    ) {
        let len = (rows * usize::try_from(per.max(1)).unwrap() + extra).min(syms.len());
        let keysyms = &syms[..len];
        let sym = if keysyms.is_empty() { sym } else { *pick.get(keysyms) };
        prop_assert_eq!(
            from_virtual(sym as i32, keysyms, per, min, &lists),
            model_from_virtual(sym as i32, keysyms, per, min, &lists)
        );
    }

    #[test]
    fn prop_to_virtual(table in table(), key in any::<i32>()) {
        let expect = if key == k("META") {
            SUPER_L as i32
        } else {
            usize::try_from(key).ok().and_then(|i| table.get(i)).map_or(0, |&s| s as i32)
        };
        prop_assert_eq!(to_virtual_of(key, &table), expect);
    }
}

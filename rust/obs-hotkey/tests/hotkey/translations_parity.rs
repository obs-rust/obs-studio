//! Tier 3 parity: the shim against the original
//! `libobs/obs-hotkey-translations.c.h` (oracle/hotkey_translations.c).
//!
//! `test_hotkey_load`, `test_wayland_hotkeys` and `test_portal_shortcuts`
//! compile the C into themselves, so they never see the Rust swap;
//! `test_hotkey_translations` sees it through a real libobs, and this file
//! compares the two directly.
//!
//! The oracle expands its templates on the original `util/dstr.c` (the C
//! dstr oracle), not on the Rust dstr port. Both sides call the same
//! recording stand-in for the store, `obs_hotkeys_store_key_translation`
//! (`super::hotkey_store`). Compared, for the same struct and `size`: every
//! store call, in order: the key, the bytes of the string, and whether the
//! pointer passed is one of the caller's members (the C passes a named
//! key's string on as it is) and which. The struct must be left as it was.
//!
//! - The layout of `struct obs_hotkeys_translations`: size, alignment and
//!   every member's offset.
//! - Every template of `test_hotkey_translations`, with every other member
//!   set, and on its own.
//! - Large templates (1 MiB, and 100000 `%1`).
//! - Random structs: each member `NULL` or a string of random pieces
//!   (`""`, padding, `%1` repeated, `%`, `1`, `\v`, printf conversions,
//!   multi-byte UTF-8, single bytes, so non-UTF-8 too), with `size` cut at
//!   every member boundary, past the struct, or at any byte (the member
//!   the cut falls in is then `NULL`), and a `NULL` struct. 2000 cases.
//!
//! Excluded: a `size` that cuts a member that is not `NULL`. Both sides
//! then copy part of a pointer, as `memcpy` does, and read through it,
//! which is undefined behaviour in each. No intentional differences.

use std::ffi::{CString, c_char};
use std::mem::{align_of, offset_of, size_of};
use std::ptr;

use obs_c_oracle::hotkey_translations::{
    oracle_hotkey_translations_layout, oracle_obs_hotkeys_set_translations_s,
};
use obs_hotkey::ffi::translations::{obs_hotkeys_set_translations_s, obs_hotkeys_translations};
use obs_hotkey::translations::{MEMBER_COUNT, Member};
use proptest::prelude::*;

use super::hotkey_store::{self, Stored};

const PTR: usize = size_of::<*const c_char>();
const STRUCT_SIZE: usize = size_of::<obs_hotkeys_translations>();
const TAIL: usize = 128;

/// The struct with bytes after it, so a `size` past the struct stays in
/// bounds (the C test's `big`).
#[repr(C)]
struct Padded {
    t: obs_hotkeys_translations,
    tail: [u8; TAIL],
}

/// One store call as compared.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Call {
    key: i32,
    name: Option<Vec<u8>>,
    /// The caller's member whose pointer was passed; `None` for a copy.
    member: Option<usize>,
}

fn calls(stored: Vec<Stored>, members: &[*const c_char; MEMBER_COUNT]) -> Vec<Call> {
    stored
        .into_iter()
        .map(|s| Call {
            key: s.key,
            name: s.name,
            member: members
                .iter()
                .position(|p| !p.is_null() && *p as usize == s.ptr),
        })
        .collect()
}

/// Runs the oracle and the shim on the same struct (or `NULL`) and `size`.
fn run_both(input: Option<&mut Padded>, size: usize) -> (Vec<Call>, Vec<Call>) {
    let (t, members, tail) = match input {
        Some(p) => {
            let members = p.t.members();
            (&raw mut p.t, members, Some(p.tail))
        }
        None => (ptr::null_mut(), [ptr::null(); MEMBER_COUNT], None),
    };
    hotkey_store::take();
    // SAFETY: `t` is null or a live struct with TAIL bytes after it, and
    // every size passed is at most STRUCT_SIZE + TAIL; each member in the
    // copied bytes is null or a NUL-terminated string (the cases make sure
    // a member a size cuts is null).
    unsafe { oracle_obs_hotkeys_set_translations_s(t.cast(), size) };
    let c = calls(hotkey_store::take(), &members);
    // SAFETY: as above.
    unsafe { obs_hotkeys_set_translations_s(t, size) };
    let rust = calls(hotkey_store::take(), &members);
    if !t.is_null() {
        // SAFETY: `t` is the live struct from above.
        let after = unsafe { &*t.cast::<Padded>() };
        assert_eq!(after.t.members(), members, "the struct is left as it was");
        assert_eq!(Some(after.tail), tail);
    }
    (c, rust)
}

/// A struct of these member strings (`None` for `NULL`).
struct Owned {
    strings: Vec<Option<CString>>,
}

impl Owned {
    fn new(values: &[Option<Vec<u8>>]) -> Self {
        assert_eq!(values.len(), MEMBER_COUNT);
        Self {
            strings: values
                .iter()
                .map(|v| v.as_ref().map(|b| CString::new(b.clone()).unwrap()))
                .collect(),
        }
    }

    fn padded(&self) -> Padded {
        let mut m = [ptr::null(); MEMBER_COUNT];
        for (slot, s) in m.iter_mut().zip(&self.strings) {
            *slot = s.as_ref().map_or(ptr::null(), |s| s.as_ptr());
        }
        Padded {
            t: obs_hotkeys_translations::from_members(m),
            tail: [0xff; TAIL],
        }
    }
}

#[track_caller]
fn assert_same(values: &[Option<Vec<u8>>], size: usize) {
    let owned = Owned::new(values);
    let mut p = owned.padded();
    let (c, rust) = run_both(Some(&mut p), size);
    assert_eq!(rust, c, "size {size}");
}

#[test]
fn layout_matches_c() {
    // SAFETY: plain C functions returning numbers.
    let c = |i: usize| unsafe { oracle_hotkey_translations_layout(i) };
    assert_eq!(STRUCT_SIZE, c(0), "sizeof");
    assert_eq!(align_of::<obs_hotkeys_translations>(), c(1), "alignof");
    macro_rules! offsets {
        ($($m:ident),* $(,)?) => {
            [$(offset_of!(obs_hotkeys_translations, $m)),*]
        };
    }
    let rust = offsets!(
        insert,
        del,
        home,
        end,
        page_up,
        page_down,
        num_lock,
        scroll_lock,
        caps_lock,
        backspace,
        tab,
        print,
        pause,
        left,
        right,
        up,
        down,
        shift,
        alt,
        control,
        meta,
        menu,
        space,
        numpad_num,
        numpad_divide,
        numpad_multiply,
        numpad_minus,
        numpad_plus,
        numpad_decimal,
        apple_keypad_num,
        apple_keypad_divide,
        apple_keypad_multiply,
        apple_keypad_minus,
        apple_keypad_plus,
        apple_keypad_decimal,
        apple_keypad_equal,
        mouse_num,
        escape,
    );
    for (i, (off, m)) in rust.iter().zip(Member::ALL).enumerate() {
        assert_eq!(*off, c(2 + i), "offsetof {}", m.name());
        assert_eq!(*off, m.index() * PTR, "{} in struct order", m.name());
    }
    assert_eq!(c(2 + MEMBER_COUNT), usize::MAX);
    // members() and from_members() follow the same order.
    let mut m = [ptr::null(); MEMBER_COUNT];
    let bytes = [0u8; MEMBER_COUNT];
    for (slot, b) in m.iter_mut().zip(&bytes) {
        *slot = ptr::from_ref(b).cast();
    }
    let t = obs_hotkeys_translations::from_members(m);
    assert_eq!(t.members(), m);
    assert_eq!(t.left, m[Member::Left.index()]);
    assert_eq!(t.escape, m[Member::Escape.index()]);
}

/// Every member set to "<prefix> <member>", templates to "... %1".
fn filled(prefix: &str) -> Vec<Option<Vec<u8>>> {
    Member::ALL
        .iter()
        .map(|m| {
            Some(match m {
                Member::NumpadNum | Member::AppleKeypadNum | Member::MouseNum => {
                    format!("{prefix} {} %1", m.name()).into_bytes()
                }
                _ => format!("{prefix} {}", m.name()).into_bytes(),
            })
        })
        .collect()
}

/// The templates of `test_hotkey_translations`.
const TEMPLATES: [&[u8]; 37] = [
    b"Numpad %1",
    b"%1 (Keypad)",
    b"Pad",
    b"%1",
    b"%1-%1",
    b"%1%1",
    b"  Pad %1\t",
    b" \t\r\nPad\r\n",
    b"  %1  ",
    b"\x0bPad\x0b",
    b"Pad  %1",
    b"%%1",
    b"%11",
    b"%2",
    b"% 1",
    b"100%",
    b"%d %s",
    b"Ziffernblock %1",
    b"\xe3\x83\x86\xe3\x83\xb3\xe3\x82\xad\xe3\x83\xbc %1",
    b"\xff\xfe %1",
    b"x",
    b"Mouse %1",
    b"Maus",
    b"Button %1 (%1)",
    b"\tM %1 \n",
    b"\xd8\xa7\xd9\x84\xd9\x81\xd8\xa3\xd8\xb1\xd8\xa9 %1",
    b"",
    b" ",
    b" \t\r\n ",
    b"  ",
    b"\t",
    b"\r\n",
    b"Links",
    b"K num %1",
    b"%n%s%1",
    b"\xe2\x82%1\xac",
    b"%",
];

#[test]
fn characterization_templates_match_c() {
    for tmpl in TEMPLATES {
        for member in [Member::NumpadNum, Member::AppleKeypadNum, Member::MouseNum] {
            let mut alone: Vec<Option<Vec<u8>>> = vec![None; MEMBER_COUNT];
            alone[member.index()] = Some(tmpl.to_vec());
            assert_same(&alone, STRUCT_SIZE);
            let mut all = filled("T");
            all[member.index()] = Some(tmpl.to_vec());
            assert_same(&all, STRUCT_SIZE);
        }
        // As every member, named keys included.
        assert_same(&vec![Some(tmpl.to_vec()); MEMBER_COUNT], STRUCT_SIZE);
    }
}

#[test]
fn whole_struct_cases_match_c() {
    let f = filled("A");
    assert_same(&f, STRUCT_SIZE);
    assert_same(&f, STRUCT_SIZE + TAIL);
    assert_same(&f, 0);
    for m in Member::ALL {
        assert_same(&f, m.index() * PTR);
    }
    assert_same(&vec![None; MEMBER_COUNT], STRUCT_SIZE);
    let (c, rust) = run_both(None, STRUCT_SIZE);
    assert_eq!((c.len(), rust.len()), (0, 0));
    let (c, rust) = run_both(None, 0);
    assert_eq!((c.len(), rust.len()), (0, 0));
    // The arrow keys are stored, as of #1673.
    let (c, _) = {
        let owned = Owned::new(&f);
        let mut p = owned.padded();
        run_both(Some(&mut p), STRUCT_SIZE)
    };
    assert!(
        c.iter()
            .any(|call| call.member == Some(Member::Down.index()))
    );
}

#[test]
fn large_templates_match_c() {
    let mut big = vec![b'a'; 1 << 20];
    big.extend_from_slice(b"%1");
    let many = b"%1".repeat(100_000);
    let padded = [&b" ".repeat(1 << 16)[..], b"x", &b"\t".repeat(1 << 16)].concat();
    for t in [big, many, padded] {
        let mut v: Vec<Option<Vec<u8>>> = vec![None; MEMBER_COUNT];
        v[Member::NumpadNum.index()] = Some(t.clone());
        v[Member::MouseNum.index()] = Some(t.clone());
        v[Member::Insert.index()] = Some(t);
        assert_same(&v, STRUCT_SIZE);
    }
}

fn piece() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        3 => Just(b"%1".to_vec()),
        1 => Just(b"%1%1%1".to_vec()),
        1 => Just(b"%".to_vec()),
        1 => Just(b"1".to_vec()),
        1 => Just(b"%%".to_vec()),
        2 => Just(b" ".to_vec()),
        1 => Just(b"\t".to_vec()),
        1 => Just(b"\r\n".to_vec()),
        1 => Just(b"\n".to_vec()),
        1 => Just(b"\x0b".to_vec()),
        1 => Just(b"\x0c".to_vec()),
        1 => Just(b"%s%n%d".to_vec()),
        1 => Just("é".as_bytes().to_vec()),
        1 => Just("ミ".as_bytes().to_vec()),
        1 => Just("\u{1f600}".as_bytes().to_vec()),
        1 => Just("\u{a0}".as_bytes().to_vec()),
        1 => Just(b"\xe2\x82".to_vec()),
        1 => Just(b"Pad".to_vec()),
        2 => (1..=255u8).prop_map(|b| vec![b]),
    ]
}

fn member_value() -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![
        2 => Just(None),
        1 => Just(Some(Vec::new())),
        4 => prop::collection::vec(piece(), 0..16).prop_map(|p| Some(p.concat())),
    ]
}

#[derive(Debug, Clone)]
enum Size {
    /// The first `n` members.
    Members(usize),
    /// Past the struct by this many bytes.
    Past(usize),
    /// Any byte count up to the struct; the member it cuts is set `NULL`.
    Bytes(usize),
}

fn size() -> impl Strategy<Value = Size> {
    prop_oneof![
        2 => Just(Size::Members(MEMBER_COUNT)),
        2 => (0..=MEMBER_COUNT).prop_map(Size::Members),
        1 => (0..=TAIL).prop_map(Size::Past),
        2 => (0..=STRUCT_SIZE).prop_map(Size::Bytes),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn random_structs_match_c(
        mut values in prop::collection::vec(member_value(), MEMBER_COUNT),
        size in size(),
        null_struct in prop::bool::weighted(0.02),
    ) {
        let size = match size {
            Size::Members(n) => n * PTR,
            Size::Past(n) => STRUCT_SIZE + n,
            Size::Bytes(n) => {
                if n % PTR != 0 {
                    values[n / PTR] = None;
                }
                n
            }
        };
        let owned = Owned::new(&values);
        let mut p = owned.padded();
        let (c, rust) = run_both((!null_struct).then_some(&mut p), size);
        prop_assert_eq!(rust, c);
    }
}

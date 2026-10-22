//! Tier 1: the safe core of `libobs/obs-context-names.c.h`.
//!
//! The first twelve tests mirror `test/cmocka/test_context_name_helpers.c`
//! one to one, with the same names (without `test_`), in the same order,
//! with the same inputs and the same expected values, and the same scripted
//! `exists()`: a set of taken names, or "taken for the first n questions",
//! with a log of every question. `dedup_many` takes 200 numbers where the C
//! takes 100000; the order of the questions is what it pins.
//!
//! Cases marked "characterized, not endorsed" are odd but real (#1788) and
//! kept as the C has them. The cases after the twelve pin the edges of the
//! safe API: the unnamed name at the ends of the counter and the four-digit
//! boundary, the candidate format, a NULL name that is taken, the counter
//! wrapping, and proptests over any bytes and any taken set.

use std::collections::HashSet;

use obs_core::context_names::{
    Lookup, NULL_STRING, UNNAMED_PREFIX, deduplicate, name_dup, suffixed_name, unnamed_name,
};
use proptest::prelude::*;

/// One question to `exists()`: `None` for a NULL name.
type Asked = Option<Vec<u8>>;

/// The scripted `exists()` of the C test.
#[derive(Default)]
struct Names {
    taken: Vec<Vec<u8>>,
    /// Taken: the first `always_taken` questions, whatever the name.
    always_taken: usize,
    asked: Vec<Asked>,
    /// Which kind of question each one was.
    kinds: Vec<bool>,
}

impl Names {
    fn taken(names: &[&[u8]]) -> Self {
        Self {
            taken: names.iter().map(|n| n.to_vec()).collect(),
            ..Self::default()
        }
    }

    fn always(n: usize) -> Self {
        Self {
            always_taken: n,
            ..Self::default()
        }
    }

    fn exists(&mut self, lookup: Lookup<'_>) -> bool {
        let n = self.asked.len();
        let (name, is_name) = match lookup {
            Lookup::Name(name) => (name.map(<[u8]>::to_vec), true),
            Lookup::Candidate(name) => (Some(name.to_vec()), false),
        };
        self.asked.push(name.clone());
        self.kinds.push(is_name);
        let Some(name) = name else {
            return false;
        };
        n < self.always_taken || self.taken.contains(&name)
    }

    /// `deduplicate(name)` with a fresh log.
    fn dedup(&mut self, name: Option<&[u8]>) -> Option<Vec<u8>> {
        self.asked.clear();
        self.kinds.clear();
        deduplicate(name, |l| self.exists(l))
    }

    fn asked(&self, i: usize) -> &[u8] {
        self.asked[i].as_deref().expect("a name, not NULL")
    }
}

// ---------------------------------------------------------------------------
// obs_context_name_deduplicate

#[test]
fn dedup_free_name() {
    let mut names = Names::taken(&[b"A", b"B 2"]);

    // A free name: None (keep the name), after one question.
    assert_eq!(names.dedup(Some(b"B")), None);
    assert_eq!(names.asked.len(), 1);
    assert_eq!(names.asked(0), b"B");
    assert_eq!(names.kinds, [true]);

    assert_eq!(names.dedup(Some(b"a")), None);
    assert_eq!(names.dedup(Some(b"A ")), None);
    assert_eq!(names.dedup(Some(b" A")), None);
    assert_eq!(names.dedup(Some(b"")), None);
}

#[test]
fn dedup_suffix() {
    {
        let mut names = Names::taken(&[b"A"]);
        assert_eq!(names.dedup(Some(b"A")).as_deref(), Some(&b"A 2"[..]));
        assert_eq!(names.asked.len(), 2);
        assert_eq!(names.asked(0), b"A");
        assert_eq!(names.asked(1), b"A 2");
        assert_eq!(names.kinds, [true, false]);
    }
    {
        // The first free number from 2 up, gaps included.
        let mut names = Names::taken(&[b"A", b"A 2", b"A 4"]);
        assert_eq!(names.dedup(Some(b"A")).as_deref(), Some(&b"A 3"[..]));
        assert_eq!(names.asked.len(), 3);
        assert_eq!(names.asked(2), b"A 3");
    }
    {
        let mut names = Names::taken(&[b"A", b"A 3"]);
        assert_eq!(names.dedup(Some(b"A")).as_deref(), Some(&b"A 2"[..]));
    }
    {
        // Only the name itself decides whether a suffix is needed.
        let mut names = Names::taken(&[b"A 2", b"A 3"]);
        assert_eq!(names.dedup(Some(b"A")), None);
    }
    {
        // " 1" and " 0" are never tried.
        let mut names = Names::taken(&[b"A", b"A 1", b"A 0"]);
        assert_eq!(names.dedup(Some(b"A")).as_deref(), Some(&b"A 2"[..]));
    }
}

/// A name is not parsed: a trailing number or " (2)" is more bytes.
#[test]
fn dedup_names_ending_in_numbers() {
    let cases: [(&[u8], &[u8]); 8] = [
        (b"Cam 2", b"Cam 2 2"),
        (b"X (2)", b"X (2) 2"),
        (b"Y2", b"Y2 2"),
        (b"Z 0", b"Z 0 2"),
        (b"N -1", b"N -1 2"),
        (b"9", b"9 2"),
        (b"Big 2147483647", b"Big 2147483647 2"),
        (b"A 02", b"A 02 2"),
    ];
    for (name, result) in cases {
        let mut names = Names::taken(&[name]);
        assert_eq!(names.dedup(Some(name)).as_deref(), Some(result));
    }

    let mut names = Names::taken(&[b"Cam 2", b"Cam 2 2"]);
    assert_eq!(
        names.dedup(Some(b"Cam 2")).as_deref(),
        Some(&b"Cam 2 3"[..])
    );
}

/// '%' is never a format, and bytes are kept as they are.
#[test]
fn dedup_bytes() {
    {
        let mut names = Names::taken(&[b"%s %d %n %%"]);
        assert_eq!(
            names.dedup(Some(b"%s %d %n %%")).as_deref(),
            Some(&b"%s %d %n %% 2"[..])
        );
    }
    {
        let mut names = Names::taken(&[b"\xff\xfe\x80", b"\xff\xfe\x80 2"]);
        assert_eq!(
            names.dedup(Some(b"\xff\xfe\x80")).as_deref(),
            Some(&b"\xff\xfe\x80 3"[..])
        );
    }
    {
        // Characterized, not endorsed: an empty name that is taken gets
        // " 2" (libobs gives a context an unnamed name first, so no table
        // holds "" and this is not reached through it).
        let mut names = Names::taken(&[b""]);
        assert_eq!(names.dedup(Some(b"")).as_deref(), Some(&b" 2"[..]));
    }
    {
        let mut names = Names::taken(&[b"\t\n"]);
        assert_eq!(names.dedup(Some(b"\t\n")).as_deref(), Some(&b"\t\n 2"[..]));
    }
}

#[test]
fn dedup_long_name() {
    let len = 1024 * 1024;
    let name = vec![b'L'; len];

    let mut names = Names::always(1);
    let result = names.dedup(Some(&name)).expect("a new name");
    assert_eq!(result.len(), len + 2);
    assert_eq!(&result[..len], &name[..]);
    assert_eq!(&result[len..], b" 2");
    assert_eq!(names.asked.len(), 2);
}

/// Taken numbers: one question per number, in order.
#[test]
fn dedup_many() {
    let mut names = Names::always(200);
    assert_eq!(names.dedup(Some(b"M")).as_deref(), Some(&b"M 201"[..]));
    assert_eq!(names.asked.len(), 201);
    assert_eq!(names.asked(0), b"M");
    assert_eq!(names.asked(1), b"M 2");
    assert_eq!(names.asked(15), b"M 16");
    for (i, asked) in names.asked.iter().enumerate().skip(1) {
        assert_eq!(asked.as_deref(), Some(format!("M {}", i + 1).as_bytes()));
    }
}

#[test]
fn dedup_param() {
    // exists() is asked for the name it was given, and the candidates.
    let mut names = Names::taken(&[b"P"]);
    assert_eq!(names.dedup(Some(b"P")).as_deref(), Some(&b"P 2"[..]));
    assert_eq!(names.asked.len(), 2);
}

/// Characterized, not endorsed: a NULL name goes to exists() as it is. The
/// exists() of obs.c (HASH_FIND_STR) does not take NULL, so obs.c never
/// asks for one: a rename to NULL takes the unnamed name without asking.
#[test]
fn dedup_null_name() {
    let mut names = Names::default();
    assert_eq!(names.dedup(None), None);
    assert_eq!(names.asked, [None]);
    assert_eq!(names.kinds, [true]);
}

// ---------------------------------------------------------------------------
// obs_context_name_dup

fn check_dup(name: Option<&[u8]>, private: bool, index: i64, expected: Option<&[u8]>, next: i64) {
    let mut counter = index;
    let result = name_dup(name, private, &mut counter);
    assert_eq!(
        result.as_deref(),
        expected,
        "{name:?} private={private} {index}"
    );
    assert_eq!(counter, next, "{name:?} private={private} {index}");
}

#[test]
fn dup_named() {
    // A name is copied, and the counter is not used.
    check_dup(Some(b"A"), false, 0, Some(b"A"), 0);
    check_dup(Some(b"A"), true, 7, Some(b"A"), 7);
    check_dup(Some(b" "), false, 0, Some(b" "), 0);
    check_dup(Some(b"%s %d"), false, 0, Some(b"%s %d"), 0);
    check_dup(Some(b"\xff\xfe\x80"), false, 0, Some(b"\xff\xfe\x80"), 0);
    check_dup(Some(b"__unnamed0000"), false, 0, Some(b"__unnamed0000"), 0);
}

#[test]
fn dup_unnamed() {
    check_dup(None, false, 0, Some(b"__unnamed0000"), 1);
    check_dup(Some(b""), false, 0, Some(b"__unnamed0000"), 1);
    check_dup(Some(b""), false, 1, Some(b"__unnamed0001"), 2);
    check_dup(None, false, 42, Some(b"__unnamed0042"), 43);
    check_dup(None, false, 9999, Some(b"__unnamed9999"), 10000);
    check_dup(None, false, 10000, Some(b"__unnamed10000"), 10001);

    // A private context keeps a NULL name and does not use the counter; an
    // empty name still gets an unnamed name.
    check_dup(None, true, 5, None, 5);
    check_dup(Some(b""), true, 5, Some(b"__unnamed0005"), 6);

    // The counter in sequence.
    let mut counter = 0;
    let a = name_dup(None, false, &mut counter);
    let b = name_dup(Some(b""), false, &mut counter);
    let c = name_dup(Some(b"C"), false, &mut counter);
    let d = name_dup(None, true, &mut counter);
    let e = name_dup(Some(b""), true, &mut counter);
    assert_eq!(a.as_deref(), Some(&b"__unnamed0000"[..]));
    assert_eq!(b.as_deref(), Some(&b"__unnamed0001"[..]));
    assert_eq!(c.as_deref(), Some(&b"C"[..]));
    assert_eq!(d, None);
    assert_eq!(e.as_deref(), Some(&b"__unnamed0002"[..]));
    assert_eq!(counter, 3);
}

/// Characterized, not endorsed: the counter is printed as a signed number,
/// the width is a minimum, and nothing bounds it. libobs starts it at 0 and
/// only counts up, so the negative values are not reached.
#[test]
fn dup_counter_range() {
    check_dup(None, false, -1, Some(b"__unnamed-001"), 0);
    check_dup(None, false, -12345, Some(b"__unnamed-12345"), -12344);
    check_dup(
        None,
        false,
        i64::MAX - 1,
        Some(b"__unnamed9223372036854775806"),
        i64::MAX,
    );
    check_dup(
        None,
        false,
        i64::MIN,
        Some(b"__unnamed-9223372036854775808"),
        i64::MIN + 1,
    );
}

#[test]
fn dup_long_name() {
    let len = 1024 * 1024;
    let name = vec![b'L'; len];
    let mut counter = 3;
    let result = name_dup(Some(&name), false, &mut counter).expect("a copy");
    assert_eq!(result, name);
    assert_eq!(counter, 3);
}

// ---------------------------------------------------------------------------
// The edges of the safe API

/// `"%04lld"`: zero-padded to four characters, the sign counted.
#[test]
fn unnamed_name_width() {
    let cases: [(i64, &str); 16] = [
        (0, "0000"),
        (1, "0001"),
        (9, "0009"),
        (10, "0010"),
        (999, "0999"),
        (1000, "1000"),
        (9999, "9999"),
        (10000, "10000"),
        (99999, "99999"),
        (-1, "-001"),
        (-99, "-099"),
        (-100, "-100"),
        (-999, "-999"),
        (-1000, "-1000"),
        (i64::MAX, "9223372036854775807"),
        (i64::MIN, "-9223372036854775808"),
    ];
    for (index, digits) in cases {
        let mut expected = UNNAMED_PREFIX.to_vec();
        expected.extend_from_slice(digits.as_bytes());
        assert_eq!(unnamed_name(index), expected, "{index}");
    }
}

/// The C's `(*unnamed_index)++` past `LLONG_MAX` wraps in the compiled C;
/// here it wraps too (libobs counts from 0, so it is not reached).
#[test]
fn dup_counter_wraps() {
    check_dup(
        None,
        false,
        i64::MAX,
        Some(b"__unnamed9223372036854775807"),
        i64::MIN,
    );
}

/// `"%s %d"`, the suffix printed signed.
#[test]
fn suffixed_name_format() {
    assert_eq!(suffixed_name(Some(b"A"), 2), b"A 2");
    assert_eq!(suffixed_name(Some(b""), 2), b" 2");
    assert_eq!(suffixed_name(Some(b"A"), 10), b"A 10");
    assert_eq!(suffixed_name(Some(b"A"), i32::MAX), b"A 2147483647");
    assert_eq!(suffixed_name(Some(b"A"), -1), b"A -1");
    assert_eq!(suffixed_name(Some(b"A"), i32::MIN), b"A -2147483648");
    assert_eq!(suffixed_name(Some(b"%d"), 3), b"%d 3");
    assert_eq!(suffixed_name(None, 2), b"(null) 2");
}

/// A NULL name that `exists()` calls taken gets `"(null) 2"`, as the C's
/// `"%s"` prints it. obs.c never asks for NULL.
#[test]
fn dedup_null_name_taken() {
    let mut asked = Vec::new();
    let result = deduplicate(None, |l| {
        asked.push(match l {
            Lookup::Name(n) => n.map(<[u8]>::to_vec),
            Lookup::Candidate(c) => Some(c.to_vec()),
        });
        asked.len() < 3
    });
    assert_eq!(result.as_deref(), Some(&b"(null) 3"[..]));
    assert_eq!(
        asked,
        [None, Some(b"(null) 2".to_vec()), Some(b"(null) 3".to_vec())]
    );
    let mut expected = NULL_STRING.to_vec();
    expected.extend_from_slice(b" 3");
    assert_eq!(result, Some(expected));
}

/// The first question is the name as given, then only candidates.
#[test]
fn dedup_question_kinds() {
    let mut names = Names::always(4);
    assert_eq!(names.dedup(Some(b"K")).as_deref(), Some(&b"K 5"[..]));
    assert_eq!(names.kinds, [true, false, false, false, false]);
}

fn bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            4 => any::<u8>().prop_filter("no NUL", |b| *b != 0),
            2 => Just(b' '),
            2 => prop::sample::select(b"0123456789".to_vec()),
        ],
        0..12,
    )
}

/// Questions as the written-out rule: the name, then `"<name> n"` from 2
/// up, until the first free one.
fn expected_questions(name: &[u8], taken: &HashSet<Vec<u8>>) -> (Option<Vec<u8>>, Vec<Vec<u8>>) {
    let mut asked = vec![name.to_vec()];
    if !taken.contains(name) {
        return (None, asked);
    }
    let mut n = 2;
    loop {
        let candidate = [name, format!(" {n}").as_bytes()].concat();
        asked.push(candidate.clone());
        if !taken.contains(&candidate) {
            return (Some(candidate), asked);
        }
        n += 1;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn dedup_against_the_rule(
        name in bytes(),
        suffixes in prop::collection::vec(1..12i32, 0..10),
        others in prop::collection::vec(bytes(), 0..4),
        name_taken in any::<bool>(),
    ) {
        let mut taken: HashSet<Vec<u8>> = others.into_iter().collect();
        for s in suffixes {
            taken.insert([&name[..], format!(" {s}").as_bytes()].concat());
        }
        if name_taken {
            taken.insert(name.clone());
        } else {
            taken.remove(&name);
        }
        let (expected, expected_asked) = expected_questions(&name, &taken);

        let mut asked = Vec::new();
        let result = deduplicate(Some(&name), |l| {
            let q = match l {
                Lookup::Name(n) => n.map(<[u8]>::to_vec).expect("not NULL"),
                Lookup::Candidate(c) => c.to_vec(),
            };
            let t = taken.contains(&q);
            asked.push(q);
            t
        });
        prop_assert_eq!(result, expected);
        prop_assert_eq!(asked, expected_asked);
    }

    #[test]
    fn dup_against_the_rule(
        name in prop::option::of(bytes()),
        private in any::<bool>(),
        index in any::<i64>(),
    ) {
        let mut counter = index;
        let result = name_dup(name.as_deref(), private, &mut counter);
        match name.as_deref() {
            None if private => {
                prop_assert_eq!(result, None);
                prop_assert_eq!(counter, index);
            }
            None | Some(b"") => {
                let mut expected = UNNAMED_PREFIX.to_vec();
                expected.extend_from_slice(format!("{index:04}").as_bytes());
                prop_assert_eq!(result, Some(expected));
                prop_assert_eq!(counter, index.wrapping_add(1));
            }
            Some(n) => {
                prop_assert_eq!(result.as_deref(), Some(n));
                prop_assert_eq!(counter, index);
            }
        }
    }
}

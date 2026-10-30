//! Tier 3: duplicate section names and duplicate keys in `config-file.c`
//! against the original C (obs-rust/obs-studio#681).
//!
//! C adds every parsed section and item with `HASH_ADD_STR` and looks it up
//! with `HASH_FIND_STR`. uthash prepends into a bucket chain and relinks the
//! chains (reversing same-key order) whenever it expands the bucket array,
//! so which duplicate a lookup returns depends on how many entries were
//! added: the newest for 1-9 duplicates, an older one after an expansion.
//! The port must return the same duplicate as C for every input, for the
//! getters and for `config_set_*`, `config_remove_value` and
//! `config_has_user_value`, which share the lookup.
//!
//! No input is excluded.

use core::ffi::{CStr, c_char};
use std::ffi::CString;

use obs_c_oracle::config_file::{
    OracleConfig, oracle_config_close, oracle_config_get_string, oracle_config_has_user_value,
    oracle_config_open, oracle_config_remove_value, oracle_config_save, oracle_config_set_string,
};
use obs_util::ffi::config_file::{
    config_close, config_data, config_get_string, config_has_user_value, config_open,
    config_remove_value, config_save, config_set_string,
};
use proptest::prelude::*;

use super::common;
use common::ScratchDir;

const SECTIONS: [&str; 3] = ["s", "t", "u"];
const KEYS: [&str; 3] = ["k", "j", "m"];

/// One `[section]` header followed by `key=value` lines.
type Block = (usize, Vec<(usize, u8)>);

#[derive(Debug, Clone)]
enum Op {
    Set(usize, usize, u8),
    Remove(usize, usize),
    Probe(usize, usize),
}

fn render(blocks: &[Block]) -> Vec<u8> {
    let mut out = String::new();
    for (n, (s, items)) in blocks.iter().enumerate() {
        out.push_str(&format!("[{}]\n", SECTIONS[*s]));
        for (k, v) in items {
            out.push_str(&format!("{}=b{n}v{v}\n", KEYS[*k]));
        }
    }
    out.into_bytes()
}

/// Many headers of one name with a key in a random subset of them, which is
/// the shape of the issue's repro.
fn arb_repeated() -> impl Strategy<Value = Vec<Block>> {
    (1usize..40, prop::collection::vec(any::<bool>(), 40)).prop_map(|(n, mask)| {
        (0..n)
            .map(|i| (0, if mask[i] { vec![(0, i as u8)] } else { vec![] }))
            .collect()
    })
}

/// One section holding the same key `n` times, mixed with other keys.
fn arb_repeated_keys() -> impl Strategy<Value = Vec<Block>> {
    prop::collection::vec(0usize..3, 1..40).prop_map(|keys| {
        vec![(
            0,
            keys.into_iter()
                .enumerate()
                .map(|(i, k)| (k, i as u8))
                .collect(),
        )]
    })
}

fn arb_mixed() -> impl Strategy<Value = Vec<Block>> {
    prop::collection::vec(
        (
            0usize..3,
            prop::collection::vec((0usize..3, any::<u8>()), 0..4),
        ),
        1..40,
    )
}

fn arb_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..3, 0usize..3, any::<u8>()).prop_map(|(s, k, v)| Op::Set(s, k, v)),
        (0usize..3, 0usize..3).prop_map(|(s, k)| Op::Remove(s, k)),
        (0usize..3, 0usize..3).prop_map(|(s, k)| Op::Probe(s, k)),
    ]
}

fn scratch(tag: &str) -> ScratchDir {
    ScratchDir::new(&format!("obs-config-dup-{tag}"))
}

fn bytes(p: *const c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        None
    } else {
        // SAFETY: `p` is a NUL-terminated string owned by the live config.
        Some(unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
    }
}

struct Pair {
    oracle: *mut OracleConfig,
    rust: *mut config_data,
}

impl Pair {
    fn open(c_path: &CString, r_path: &CString) -> Pair {
        let mut o: *mut OracleConfig = core::ptr::null_mut();
        let mut r: *mut config_data = core::ptr::null_mut();
        // SAFETY: out-pointers and path strings are live for the calls.
        let oc = unsafe { oracle_config_open(&mut o, c_path.as_ptr(), 0) };
        // SAFETY: same as above.
        let rc = unsafe { config_open(&mut r, r_path.as_ptr(), 0) };
        assert_eq!((oc, rc), (0, 0), "open");
        Pair { oracle: o, rust: r }
    }

    fn probe(&self, s: usize, k: usize) {
        let (sc, kc) = (
            CString::new(SECTIONS[s]).unwrap(),
            CString::new(KEYS[k]).unwrap(),
        );
        // SAFETY: live configs; the C strings outlive the calls.
        let (o, r, oh, rh) = unsafe {
            (
                bytes(oracle_config_get_string(
                    self.oracle,
                    sc.as_ptr(),
                    kc.as_ptr(),
                )),
                bytes(config_get_string(self.rust, sc.as_ptr(), kc.as_ptr())),
                oracle_config_has_user_value(self.oracle, sc.as_ptr(), kc.as_ptr()),
                config_has_user_value(self.rust, sc.as_ptr(), kc.as_ptr()),
            )
        };
        assert_eq!(r, o, "get_string [{}] {}", SECTIONS[s], KEYS[k]);
        assert_eq!(rh, oh, "has_user_value [{}] {}", SECTIONS[s], KEYS[k]);
    }

    fn probe_all(&self) {
        for s in 0..SECTIONS.len() {
            for k in 0..KEYS.len() {
                self.probe(s, k);
            }
        }
    }

    fn apply(&self, op: &Op) {
        match *op {
            Op::Set(s, k, v) => {
                let (sc, kc) = (
                    CString::new(SECTIONS[s]).unwrap(),
                    CString::new(KEYS[k]).unwrap(),
                );
                let vc = CString::new(format!("set{v}")).unwrap();
                // SAFETY: live configs; the C strings outlive the calls.
                unsafe {
                    oracle_config_set_string(self.oracle, sc.as_ptr(), kc.as_ptr(), vc.as_ptr());
                    config_set_string(self.rust, sc.as_ptr(), kc.as_ptr(), vc.as_ptr());
                }
            }
            Op::Remove(s, k) => {
                let (sc, kc) = (
                    CString::new(SECTIONS[s]).unwrap(),
                    CString::new(KEYS[k]).unwrap(),
                );
                // SAFETY: live configs; the C strings outlive the calls.
                let (o, r) = unsafe {
                    (
                        oracle_config_remove_value(self.oracle, sc.as_ptr(), kc.as_ptr()),
                        config_remove_value(self.rust, sc.as_ptr(), kc.as_ptr()),
                    )
                };
                assert_eq!(r, o, "remove_value [{}] {}", SECTIONS[s], KEYS[k]);
            }
            Op::Probe(s, k) => self.probe(s, k),
        }
        self.probe_all();
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        // SAFETY: each config is closed exactly once, here.
        unsafe { oracle_config_close(self.oracle) };
        // SAFETY: same as above.
        unsafe { config_close(self.rust) };
    }
}

fn run(blocks: &[Block], ops: &[Op]) {
    let text = render(blocks);
    let (c_dir, r_dir) = (scratch("c"), scratch("r"));
    let (c_path, r_path) = (c_dir.join("cfg.ini"), r_dir.join("cfg.ini"));
    std::fs::write(&c_path, &text).unwrap();
    std::fs::write(&r_path, &text).unwrap();
    let cp = CString::new(c_path.as_os_str().as_encoded_bytes()).unwrap();
    let rp = CString::new(r_path.as_os_str().as_encoded_bytes()).unwrap();
    let pair = Pair::open(&cp, &rp);
    pair.probe_all();
    for op in ops {
        pair.apply(op);
    }
    // SAFETY: both configs are live.
    let (os, rs) = unsafe { (oracle_config_save(pair.oracle), config_save(pair.rust)) };
    assert_eq!(rs, os, "save");
    assert_eq!(
        std::fs::read(&r_path).unwrap(),
        std::fs::read(&c_path).unwrap(),
        "saved bytes (shows which duplicate each set/remove touched)"
    );
    drop(pair);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn parity_duplicate_sections(
        blocks in arb_repeated(),
        ops in prop::collection::vec(arb_op(), 0..12),
    ) {
        run(&blocks, &ops);
    }

    #[test]
    fn parity_duplicate_keys(
        blocks in arb_repeated_keys(),
        ops in prop::collection::vec(arb_op(), 0..12),
    ) {
        run(&blocks, &ops);
    }

    #[test]
    fn parity_duplicates_mixed(
        blocks in arb_mixed(),
        ops in prop::collection::vec(arb_op(), 0..12),
    ) {
        run(&blocks, &ops);
    }
}

/// The issue's minimal repro: ten `[s]` headers, `k=v` only in the last.
#[test]
fn ten_sections_key_only_in_last() {
    let mut blocks: Vec<Block> = (0..10).map(|_| (0, vec![])).collect();
    blocks[9].1.push((0, 7));
    run(&blocks, &[]);
}

/// Every duplicate count up to 120 (past several expansions and uthash's
/// `noexpand` cut-off), for sections and for keys.
#[test]
fn every_duplicate_count_up_to_120() {
    for n in 1..=120usize {
        let secs: Vec<Block> = (0..n).map(|i| (0, vec![(0, i as u8)])).collect();
        run(&secs, &[Op::Set(0, 0, 1), Op::Remove(0, 0)]);
        let keys: Vec<Block> = vec![(0, (0..n).map(|i| (0, i as u8)).collect())];
        run(
            &keys,
            &[Op::Set(0, 0, 1), Op::Remove(0, 0), Op::Remove(0, 0)],
        );
    }
}

/// Hundreds of keys, each repeated, so chains fill through hash collisions
/// of different keys (checks the `HASH_SFH` port and the multi-key chain
/// bookkeeping), compared through lookups and the saved bytes.
#[test]
fn many_distinct_and_repeated_keys() {
    for (distinct, total) in [(7usize, 150usize), (50, 400), (300, 600), (600, 650)] {
        let mut text = String::from("[s]\n");
        for i in 0..total {
            text.push_str(&format!("key{}=v{i}\n", (i * 7) % distinct));
        }
        for i in 0..distinct {
            text.push_str(&format!("[sec{}]\nk=v{i}\n", i % 40));
        }
        let (c_dir, r_dir) = (scratch("c"), scratch("r"));
        let (c_path, r_path) = (c_dir.join("cfg.ini"), r_dir.join("cfg.ini"));
        std::fs::write(&c_path, &text).unwrap();
        std::fs::write(&r_path, &text).unwrap();
        let cp = CString::new(c_path.as_os_str().as_encoded_bytes()).unwrap();
        let rp = CString::new(r_path.as_os_str().as_encoded_bytes()).unwrap();
        let pair = Pair::open(&cp, &rp);
        let get = |s: &str, k: &str| {
            let (s, k) = (CString::new(s).unwrap(), CString::new(k).unwrap());
            // SAFETY: live configs; the C strings outlive the calls.
            unsafe {
                (
                    bytes(oracle_config_get_string(
                        pair.oracle,
                        s.as_ptr(),
                        k.as_ptr(),
                    )),
                    bytes(config_get_string(pair.rust, s.as_ptr(), k.as_ptr())),
                )
            }
        };
        for i in 0..distinct {
            let (o, r) = get("s", &format!("key{i}"));
            assert_eq!(r, o, "[s] key{i} ({distinct} distinct, {total} lines)");
        }
        for i in 0..40 {
            let (o, r) = get(&format!("sec{i}"), "k");
            assert_eq!(r, o, "[sec{i}] k");
        }
        drop(pair);
    }
}

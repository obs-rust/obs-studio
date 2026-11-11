//! Original-C differential coverage. Populated-list NULL getter is excluded as C UB.
//! All arrays and byte strings are bounded; no test-owned threads.
use core::ffi::{c_char, c_int, c_void};
use obs_source::ffi::source_module::{obs_source_get_module, obs_source_load_state};
use proptest::prelude::*;
use std::sync::Mutex;
static MODULES: Mutex<()> = Mutex::new(());
type Get = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type State = unsafe extern "C" fn(*const c_char) -> c_int;
unsafe extern "C" {
    fn oracle_obs_source_get_module(id: *const c_char) -> *mut c_void;
    fn oracle_obs_source_load_state(id: *const c_char) -> c_int;
    fn oracle_source_module_run(
        get: Get,
        state: State,
        count: usize,
        split: usize,
        counts: *const usize,
        ids: *const *const c_char,
        states: *const c_int,
        query: *const c_char,
        out: *mut usize,
    );
    fn oracle_source_module_layout(out: *mut usize);
}
fn check(modules: &[(Vec<Vec<u8>>, i32)], split: usize, query: Option<&[u8]>) {
    let _lock = MODULES.lock().unwrap_or_else(|e| e.into_inner());
    let counts: Vec<_> = modules.iter().map(|(ids, _)| ids.len()).collect();
    let states: Vec<_> = modules.iter().map(|(_, s)| *s).collect();
    let strings: Vec<_> = modules
        .iter()
        .flat_map(|(ids, _)| ids)
        .map(|id| {
            let mut bytes = id.clone();
            bytes.push(0);
            bytes
        })
        .collect();
    let ids: Vec<_> = strings.iter().map(|id| id.as_ptr().cast()).collect();
    let query = query.map(|id| {
        let mut bytes = id.to_vec();
        bytes.push(0);
        bytes
    });
    let query = query
        .as_ref()
        .map_or(core::ptr::null(), |s| s.as_ptr().cast());
    let mut rust = [0; 9];
    let mut c = [0; 9];
    // SAFETY: the serialized C fixture owns real module layouts and roots. All input
    // arrays and NUL-terminated strings live through both calls; outputs have nine slots.
    unsafe {
        oracle_source_module_run(
            obs_source_get_module,
            obs_source_load_state,
            modules.len(),
            split,
            counts.as_ptr(),
            ids.as_ptr(),
            states.as_ptr(),
            query,
            rust.as_mut_ptr(),
        );
        oracle_source_module_run(
            oracle_obs_source_get_module,
            oracle_obs_source_load_state,
            modules.len(),
            split,
            counts.as_ptr(),
            ids.as_ptr(),
            states.as_ptr(),
            query,
            c.as_mut_ptr(),
        );
    }
    assert_eq!(rust, c);
    for observation in rust.chunks_exact(3) {
        assert_eq!(observation[2], 1);
    }
    assert_eq!(rust[0..3], rust[3..6]);
    assert_eq!(rust[0..3], rust[6..9]);
}
fn module(ids: &[&[u8]], state: i32) -> (Vec<Vec<u8>>, i32) {
    (ids.iter().map(|id| id.to_vec()).collect(), state)
}
#[test]
fn characterized_source_module_edges() {
    for split in 0..=2 {
        let empty = [module(&[], 1), module(&[], 3)];
        for id in [
            None,
            Some(b"".as_slice()),
            Some(b"scene"),
            Some(b"group"),
            Some(b"unknown"),
            Some(b"Scene"),
            Some(b"groups"),
        ] {
            check(&empty, split, id);
        }
    }
    for state in 0..7 {
        let modules = [
            module(&[], 1),
            module(
                &[b"other", b"duplicate", b"duplicate", b"enabled-tail"],
                state,
            ),
            module(&[], 3),
            module(
                &[
                    b"duplicate",
                    b"disabled-tail",
                    b"",
                    b"x\xff",
                    b"scene",
                    b"group",
                ],
                state,
            ),
        ];
        for split in 0..=4 {
            for id in [
                b"duplicate".as_slice(),
                b"enabled-tail",
                b"disabled-tail",
                b"Duplicate",
                b"duplicat",
                b"",
                b"x\xff",
                b"x\xff\0z",
                b"scene",
                b"group",
                b"not-in-any-module",
            ] {
                check(&modules, split, Some(id));
            }
            check(&modules, split, None); // state only when IDs are populated
        }
    }
}
#[test]
fn real_c_load_state_representation() {
    let mut values = [0; 9];
    // SAFETY: C writes nine enum measurements into this nine-element array.
    unsafe { oracle_source_module_layout(values.as_mut_ptr()) };
    assert_eq!(
        values,
        [size_of::<c_int>(), align_of::<c_int>(), 0, 1, 2, 3, 4, 5, 6]
    );
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn generated_source_modules_match_original_c(
        modules in prop::collection::vec((prop::collection::vec(prop::collection::vec(any::<u8>(), 0..24), 0..8), 0i32..7), 0..12),
        split in any::<usize>(), query in prop::option::of(prop::collection::vec(any::<u8>(), 0..24)),
        pick_registered in any::<bool>(), pick in any::<usize>(),
    ) {
        let ids: Vec<_> = modules.iter().flat_map(|(ids, _)| ids).collect();
        let query = if pick_registered && !ids.is_empty() { Some(ids[pick % ids.len()].as_slice()) } else { query.as_deref() };
        check(&modules, split % (modules.len() + 1), query);
    }
}

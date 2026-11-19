//! Original C vs ABI shim vs safe core. No-core and successful NULL output
//! are outside the C contract. No intentional differences.
use core::ffi::c_char;
use obs_core::{ffi::type_catalog_enum::*, type_catalog_enum};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::cell::Cell;

type Query = unsafe extern "C" fn(usize, *mut *const c_char) -> bool;
unsafe extern "C" {
    fn oracle_type_catalog_enum_check(
        ids: *const *const c_char,
        counts: *const usize,
        index: usize,
        queries: *const Query,
        safe_ids: *const *const c_char,
        safe_results: *const bool,
    ) -> bool;
}
fn check(catalogs: &[Vec<Vec<u8>>], index: usize) {
    let mut ids = [std::ptr::null(); 54];
    let counts: [usize; 6] = std::array::from_fn(|c| catalogs[c].len());
    let sentinels = [
        c"input untouched",
        c"filter untouched",
        c"transition untouched",
        c"output untouched",
        c"encoder untouched",
        c"service untouched",
    ];
    for c in 0..6 {
        for (i, id) in catalogs[c].iter().enumerate() {
            ids[c * 8 + i] = id.as_ptr().cast();
        }
        ids[48 + c] = sentinels[c].as_ptr();
    }
    let queries: [Query; 6] = [
        obs_enum_input_types,
        obs_enum_filter_types,
        obs_enum_transition_types,
        obs_enum_output_types,
        obs_enum_encoder_types,
        obs_enum_service_types,
    ];
    let output: [Cell<*const c_char>; 6] = std::array::from_fn(|c| Cell::new(ids[48 + c]));
    let results: [bool; 6] = std::array::from_fn(|c| {
        type_catalog_enum::enumerate(counts[c], index, |i| ids[c * 8 + i], |id| output[c].set(id))
    });
    let output: [*const c_char; 6] = output.map(Cell::into_inner);
    let before = catalogs.to_vec();
    // SAFETY: six catalogs each have at most eight live byte arrays. C never
    // dereferences strings; it compares the exact borrowed pointers. The child
    // process exclusively lends the existing global fixture and C restores it.
    assert!(unsafe {
        oracle_type_catalog_enum_check(
            ids.as_ptr(),
            counts.as_ptr(),
            index,
            queries.as_ptr(),
            output.as_ptr(),
            results.as_ptr(),
        )
    });
    assert_eq!(catalogs, before);
}
#[test]
fn type_catalog_enum_matches_original_c() {
    const CHILD: &str = "OBS_TYPE_CATALOG_ENUM_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let selector = format!(
            "{}::type_catalog_enum_matches_original_c",
            module_path!().split_once("::").unwrap().1
        );
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &selector, "--test-threads=1"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let empty = vec![vec![]; 6];
    let catalogs: Vec<_> = (0..6)
        .map(|c| (0..=c).map(|i| vec![c as u8, i as u8, 255, 0]).collect())
        .collect();
    for index in [0, 1, 2, 3, 4, 5, 6, 7, 8, u32::MAX as usize, usize::MAX] {
        check(&empty, index);
        check(&catalogs, index);
        for c in 0..6 {
            let mut one = empty.clone();
            one[c] = catalogs[c].clone();
            check(&one, index);
        }
    }
    let strategy = (
        prop::collection::vec(
            prop::collection::vec(prop::collection::vec(any::<u8>(), 1..25), 0..9),
            6,
        ),
        any::<usize>(),
    );
    TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..Config::default()
    })
    .run(&strategy, |(catalogs, index)| {
        check(&catalogs, index);
        for i in 0..=8 {
            check(&catalogs, i);
        }
        check(&catalogs, usize::MAX);
        Ok(())
    })
    .unwrap();
}

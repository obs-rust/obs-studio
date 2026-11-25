//! Compare the safe traversal and ABI with the complete extracted C body.
//! Missing non-NULL IDs are excluded from the asserting C domain, not broadened.
use core::ffi::c_char;
use obs_core::{
    ffi::latest_input_type_id::obs_get_latest_input_type_id, latest_input_type_id::latest,
};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::ffi::CString;

unsafe extern "C" {
    fn oracle_latest_input_type_id_check(
        ids: *const *const c_char,
        names: *const *const c_char,
        versions: *const u32,
        count: usize,
        query: *const c_char,
        rust: unsafe extern "C" fn(*const c_char) -> *const c_char,
        selected: usize,
    ) -> bool;
}

fn check(entries: &[(Vec<u8>, u32)], query: Option<&[u8]>) {
    let names: Vec<_> = entries
        .iter()
        .map(|(s, _)| CString::new(s.as_slice()).unwrap())
        .collect();
    let ids: Vec<_> = (0..entries.len())
        .map(|i| CString::new(format!("borrowed_{i}")).unwrap())
        .collect();
    let versions: Vec<_> = entries.iter().map(|(_, v)| *v).collect();
    let selected = latest(query, entries.len(), |i| {
        (names[i].to_bytes(), versions[i], i)
    });
    let eligible = query.is_some_and(|q| {
        entries
            .iter()
            .any(|(name, version)| name == q && *version <= i32::MAX as u32)
    });
    if query.is_some() && !eligible {
        return; // The original asserts here; release misses have external Tier 2 coverage.
    }
    assert_eq!(selected.is_some(), eligible);
    let query = query.map(|q| CString::new(q).unwrap());
    let name_ptrs: Vec<_> = names.iter().map(|s| s.as_ptr()).collect();
    let id_ptrs: Vec<_> = ids.iter().map(|s| s.as_ptr()).collect();
    // SAFETY: at most eight live, NUL-terminated names/IDs and scalar versions.
    // The exclusively re-executed child lends the fixture core; C restores it.
    assert!(unsafe {
        oracle_latest_input_type_id_check(
            id_ptrs.as_ptr(),
            name_ptrs.as_ptr(),
            versions.as_ptr(),
            entries.len(),
            query.as_ref().map_or(std::ptr::null(), |q| q.as_ptr()),
            obs_get_latest_input_type_id,
            selected.unwrap_or(usize::MAX),
        )
    });
}

#[test]
fn latest_input_type_id_matches_original_c() {
    const CHILD: &str = "OBS_LATEST_INPUT_TYPE_ID_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let selector = format!(
            "{}::latest_input_type_id_matches_original_c",
            module_path!().split_once("::").unwrap().1
        );
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &selector, "--test-threads=1"])
                .env(CHILD, "1")
                .status()
                .unwrap()
                .success()
        );
        return;
    }
    check(&[], None);
    let fixed = [
        (b"camera".to_vec(), 0x8000_0000),
        (b"camera".to_vec(), 0),
        (b"other".to_vec(), 99),
        (b"camera".to_vec(), u32::MAX),
        (b"camera".to_vec(), 9),
        (b"camera".to_vec(), 9),
        (b"camera".to_vec(), i32::MAX as u32),
        (b"camera".to_vec(), 0x8000_0001),
    ];
    for n in 2..=8 {
        check(&fixed[..n], Some(b"camera"));
        check(&fixed[..n], None);
        let reversed: Vec<_> = fixed[..n].iter().cloned().rev().collect();
        check(&reversed, Some(b"camera"));
    }
    let name = prop::collection::vec(1u8..=255, 0..17);
    let version = prop_oneof![
        Just(0),
        Just(u32::MAX),
        Just(0x8000_0000),
        Just(i32::MAX as u32),
        any::<u32>()
    ];
    let strategy = (prop::collection::vec((name.clone(), version), 0..8), name);
    TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    })
    .run(&strategy, |(mut entries, query)| {
        entries.push((query.clone(), 0)); // Keep all generated queries in the debug C domain.
        check(&entries, Some(&query));
        for i in 0..entries.len() {
            check(&entries, Some(&entries[i].0));
        }
        check(&entries, None);
        Ok(())
    })
    .unwrap();
}

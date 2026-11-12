//! Tier 3 calls the production exports and original C over real-header fixtures.
//! No-core calls and successful source calls with NULL output are outside the
//! characterized domain. There are no intentional differences from C.
use core::ffi::c_char;
use obs_core::ffi::source_type_enum::{obs_enum_input_types2, obs_enum_source_types};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

type SourceQuery = unsafe extern "C" fn(usize, *mut *const c_char) -> bool;
type InputQuery = unsafe extern "C" fn(usize, *mut *const c_char, *mut *const c_char) -> bool;
unsafe extern "C" {
    fn oracle_source_type_enum_check(
        source_ids: *const *const c_char,
        input_ids: *const *const c_char,
        unversioned_ids: *const *const c_char,
        source_count: usize,
        input_count: usize,
        index: usize,
        source: SourceQuery,
        input: InputQuery,
    ) -> bool;
    fn oracle_source_type_enum_layout(out: *mut usize);
}

fn check(sources: &[Vec<u8>], inputs: &[(Vec<u8>, Vec<u8>)], index: usize) {
    let terminated = |bytes: &[u8]| {
        let mut bytes = bytes.to_vec();
        bytes.push(0);
        bytes
    };
    let sources: Vec<_> = sources.iter().map(|s| terminated(s)).collect();
    let inputs: Vec<_> = inputs
        .iter()
        .map(|(id, unversioned)| (terminated(id), terminated(unversioned)))
        .collect();
    let source_ptrs: Vec<_> = sources.iter().map(|s| s.as_ptr().cast()).collect();
    let input_ptrs: Vec<_> = inputs.iter().map(|s| s.0.as_ptr().cast()).collect();
    let unversioned_ptrs: Vec<_> = inputs.iter().map(|s| s.1.as_ptr().cast()).collect();
    let before = (sources.clone(), inputs.clone());
    // SAFETY: bounded arrays and their strings stay live. The isolated process
    // lends the existing core-accessor fixture to C for the duration of both
    // exports. The fixture checks pointer identity, masks, aliases, failure
    // sentinels and byte-for-byte registry/core immutability, then restores it.
    assert!(unsafe {
        oracle_source_type_enum_check(
            source_ptrs.as_ptr(),
            input_ptrs.as_ptr(),
            unversioned_ptrs.as_ptr(),
            sources.len(),
            inputs.len(),
            index,
            obs_enum_source_types,
            obs_enum_input_types2,
        )
    });
    assert_eq!((sources, inputs), before);
}

#[test]
fn real_header_boundary_types() {
    let mut layout = [0; 7];
    // SAFETY: C writes seven measurements from the actual private headers.
    unsafe { oracle_source_type_enum_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<usize>(),
            align_of::<usize>(),
            size_of::<*const c_char>(),
            align_of::<*const c_char>(),
            size_of::<*const c_char>(),
            align_of::<bool>(),
            size_of::<bool>()
        ]
    );
}

#[test]
fn source_type_enum_matches_original_c() {
    const CHILD: &str = "OBS_SOURCE_TYPE_ENUM_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let selector = format!(
            "{}::source_type_enum_matches_original_c",
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
    let sources = [
        b"enum_input".to_vec(),
        b"enum_filter".to_vec(),
        b"enum_input_v3".to_vec(),
        b"enum_transition".to_vec(),
        b"enum_scene".to_vec(),
        b"\xff\0raw".to_vec(),
        vec![],
    ];
    let inputs = [
        (sources[0].clone(), sources[0].clone()),
        (sources[2].clone(), sources[0].clone()),
    ];
    for index in [0, 1, 2, 3, 4, 5, 6, 7, 8, u32::MAX as usize, usize::MAX] {
        check(&sources, &inputs, index);
        check(&[], &[], index);
        check(&sources, &[], index);
        check(&[], &inputs, index);
    }
    let bytes = prop::collection::vec(any::<u8>(), 0..24);
    let strategy = (
        prop::collection::vec(bytes.clone(), 0..9),
        prop::collection::vec((bytes.clone(), bytes), 0..9),
        any::<usize>(),
    );
    TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..Config::default()
    })
    .run(&strategy, |(sources, inputs, index)| {
        check(&sources, &inputs, index);
        for index in 0..=sources.len().max(inputs.len()) {
            check(&sources, &inputs, index);
        }
        check(&sources, &inputs, usize::MAX);
        Ok(())
    })
    .unwrap();
}

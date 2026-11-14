//! Tier 3: original extracted C, real-header fields, and thread-local fixtures.
//! NULL IDs are covered only with an empty registry, as characterized by C.
use core::{
    ffi::{c_char, c_uint, c_void},
    mem::{align_of, size_of},
    ptr,
};
use obs_source::ffi::source_theme_icon::{obs_source_get_dark_icon, obs_source_get_light_icon};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_char) -> *const c_char;
type Callback = unsafe extern "C" fn(*mut c_void) -> *const c_char;
unsafe extern "C" {
    fn oracle_obs_source_get_dark_icon(id: *const c_char) -> *const c_char;
    fn oracle_obs_source_get_light_icon(id: *const c_char) -> *const c_char;
    fn oracle_source_theme_icon_run(
        getter: Getter,
        query: *const c_char,
        ids: *const *const c_char,
        icons: *const c_uint,
        count: usize,
        flags: c_uint,
        data: *mut c_void,
        dark: *const c_char,
        light: *const c_char,
        observed: *mut usize,
    ) -> *const c_char;
    fn oracle_source_theme_icon_layout(layout: *mut usize) -> bool;
}

fn check(
    ids: &[Vec<u8>],
    icons: &[u32],
    query: Option<&[u8]>,
    flags: u32,
    null_data: bool,
    results: [Option<&[u8]>; 2],
) {
    assert_eq!(ids.len(), icons.len());
    let ids: Vec<_> = ids
        .iter()
        .map(|id| {
            assert_eq!(id.last(), Some(&0));
            id.as_ptr().cast()
        })
        .collect();
    let query: *const c_char = query.map_or(ptr::null(), |q| {
        assert!(q.contains(&0));
        q.as_ptr().cast()
    });
    assert!(!query.is_null() || ids.is_empty());
    let mut token = 0u8;
    let data = if null_data {
        ptr::null_mut()
    } else {
        ptr::from_mut(&mut token).cast()
    };
    let results = results.map(|r| r.map_or(ptr::null(), |s| s.as_ptr().cast()));
    for (index, (rust, c)) in [
        (
            obs_source_get_dark_icon as Getter,
            oracle_obs_source_get_dark_icon as Getter,
        ),
        (
            obs_source_get_light_icon as Getter,
            oracle_obs_source_get_light_icon as Getter,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut rust_events = [0; 4];
        let mut c_events = [0; 4];
        // SAFETY: arrays, terminated IDs, token, and callback results remain
        // live through both synchronous runs. The fixture is thread-local and
        // restores its previous value; NULL query is used only for zero entries.
        let (got, expected) = unsafe {
            (
                oracle_source_theme_icon_run(
                    rust,
                    query,
                    ids.as_ptr(),
                    icons.as_ptr(),
                    ids.len(),
                    flags,
                    data,
                    results[0],
                    results[1],
                    rust_events.as_mut_ptr(),
                ),
                oracle_source_theme_icon_run(
                    c,
                    query,
                    ids.as_ptr(),
                    icons.as_ptr(),
                    ids.len(),
                    flags,
                    data,
                    results[0],
                    results[1],
                    c_events.as_mut_ptr(),
                ),
            )
        };
        assert_eq!(got, expected);
        assert_eq!(rust_events, c_events);
        // A NULL asked ID reads no registry, so its one lookup is not observed.
        let lookups = if query.is_null() { 0..=1 } else { 1..=2 };
        assert!(lookups.contains(&rust_events[0]));
        if rust_events[index + 1] == 1 {
            assert_eq!(got, results[index]);
            assert_eq!(rust_events[3], 1);
        } else {
            assert!(got.is_null());
        }
        assert_eq!(rust_events[2 - index], 0);
    }
}

#[test]
fn characterization_edges_and_real_header_layout() {
    let mut layout = [0; 4];
    // SAFETY: four writable slots; C checks the actual adapter values too.
    assert!(unsafe { oracle_source_theme_icon_layout(layout.as_mut_ptr()) });
    assert_eq!(
        layout,
        [
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>(),
            size_of::<Option<Callback>>(),
            align_of::<Option<Callback>>()
        ]
    );
    check(&[], &[], None, 3, false, [None, None]);
    let ids = vec![
        b"first\0".to_vec(),
        b"first\0".to_vec(),
        b"first-suffix\0".to_vec(),
        b"\0".to_vec(),
        vec![0xfe, 0x81, 0],
    ];
    let values: [Option<&[u8]>; 5] = [
        None,
        Some(b"\0"),
        Some(b"../Mixed Case.svg\0"),
        Some(b"../Mixed Case.svg\0"),
        Some(&[0xfe, 0x81, 0]),
    ];
    for kind in 0..=14 {
        for flags in 0..8 {
            for null_data in [false, true] {
                for i in 0..values.len() {
                    for query in [
                        b"first\0ignored".as_slice(),
                        b"First\0",
                        b"firs\0",
                        b"first-suffix\0",
                        b"\0",
                        &[0xfe, 0x81, 0],
                    ] {
                        check(
                            &ids,
                            &[kind, 0, 0, 13, 13],
                            Some(query),
                            flags,
                            null_data,
                            [values[i], values[4 - i]],
                        );
                    }
                }
            }
        }
    }
    // Distinct allocations with identical text must preserve pointer identity.
    let dark = b"same.svg\0".to_vec();
    let light = b"same.svg\0".to_vec();
    assert_ne!(dark.as_ptr(), light.as_ptr());
    check(
        &ids,
        &[13, 0, 0, 13, 13],
        Some(b"first\0"),
        3,
        false,
        [Some(&dark), Some(&light)],
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_registry_and_callbacks(entries in prop::collection::vec((prop::collection::vec(1u8..=255, 0..16), 0u32..32), 0..12),
        query in prop::collection::vec(1u8..=255, 0..16), flags in 0u32..8, null_data in any::<bool>(),
        result in prop::collection::vec(any::<u8>(), 0..32), select in any::<usize>()) {
        let ids: Vec<_> = entries.iter().map(|(id,_)| { let mut id = id.clone(); id.push(0); id }).collect();
        let icons: Vec<_> = entries.iter().map(|(_,icon)| *icon).collect();
        let mut query = query;
        query.push(0);
        let query = if ids.is_empty() || select.is_multiple_of(2) { query.as_slice() } else { &ids[select % ids.len()] };
        check(&ids, &icons, Some(query), flags, null_data, [Some(&result), None]);
    }
}

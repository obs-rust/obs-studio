//! Original-C parity. No-core queries, populated-registry NULL IDs and missing
//! name callbacks on name queries are C UB and excluded, not newly tolerated.
use core::ffi::c_char;
use obs_encoder::ffi::type_info::{Fields, obs_encoder_get_display_name, obs_get_encoder_codec};
use proptest::prelude::*;

type Query = unsafe extern "C" fn(*const c_char) -> *const c_char;
unsafe extern "C" {
    fn oracle_encoder_type_info_check(
        ids: *const *const c_char,
        codecs: *const *const c_char,
        count: usize,
        query: *const c_char,
        results: *const *const c_char,
        result_count: usize,
        null_data: u32,
        names: bool,
        name: Query,
        codec: Query,
    ) -> bool;
    fn oracle_encoder_type_info_layout(out: *mut usize);
    fn oracle_encoder_type_info_mutable(name: Query, codec: Query) -> bool;
}
fn terminated(bytes: &[u8]) -> Vec<u8> {
    let mut bytes = bytes.to_vec();
    bytes.push(0);
    bytes
}
fn check(
    entries: &[(Vec<u8>, Option<Vec<u8>>)],
    query: Option<&[u8]>,
    null_data: u32,
    names: bool,
) {
    assert!(entries.len() <= 8);
    assert!(entries.is_empty() || query.is_some());
    let ids: Vec<_> = entries.iter().map(|(id, _)| terminated(id)).collect();
    let codecs: Vec<_> = entries
        .iter()
        .map(|(_, c)| c.as_ref().map(|s| terminated(s)))
        .collect();
    let id_ptrs: Vec<_> = ids.iter().map(|s| s.as_ptr().cast()).collect();
    let codec_ptrs: Vec<_> = codecs
        .iter()
        .map(|s| s.as_ref().map_or(core::ptr::null(), |s| s.as_ptr().cast()))
        .collect();
    let query = query.map(terminated);
    let query = query
        .as_ref()
        .map_or(core::ptr::null(), |s| s.as_ptr().cast());
    let first = terminated(b"same display name");
    let second = terminated(b"same display name");
    assert_ne!(first.as_ptr(), second.as_ptr());
    let results = [
        first.as_ptr().cast(),
        second.as_ptr().cast(),
        c"".as_ptr(),
        c"n\xff".as_ptr(),
        core::ptr::null(),
        first.as_ptr().cast(),
    ];
    // SAFETY: real C layouts and thread-local roots belong to the fixture. All
    // arrays/strings live through the call, sizes are bounded, and NULL IDs only
    // reach an empty registry. Missing callbacks are queried only for codecs.
    assert!(unsafe {
        oracle_encoder_type_info_check(
            id_ptrs.as_ptr(),
            codec_ptrs.as_ptr(),
            entries.len(),
            query,
            results.as_ptr(),
            results.len(),
            null_data,
            names,
            obs_encoder_get_display_name,
            obs_get_encoder_codec,
        )
    });
}
#[test]
fn real_header_type_info_snapshot_layout() {
    let mut c = [0; 5];
    // SAFETY: C writes exactly five size/offset measurements from its real header.
    unsafe { oracle_encoder_type_info_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            core::mem::offset_of!(Fields, get_name),
            core::mem::offset_of!(Fields, type_data),
            core::mem::offset_of!(Fields, codec)
        ]
    );
}
#[test]
fn characterized_encoder_type_info_edges() {
    check(&[], None, 0, true);
    for id in [b"".as_slice(), b"unknown-encoder-type"] {
        check(&[], Some(id), 0, true);
    }
    let entries = vec![
        (b"prefix".to_vec(), Some(b"prefix-codec".to_vec())),
        (b"duplicate".to_vec(), Some(b"h264".to_vec())),
        (b"duplicate".to_vec(), Some(b"h264".to_vec())),
        (
            b"duplicate-suffix".to_vec(),
            Some(b"aac;opus;;aac;".to_vec()),
        ),
        (b"e\xff\0".to_vec(), Some(b"v\xfe;\0x\0".to_vec())),
        (b"".to_vec(), Some(vec![])),
        (b"null-codec".to_vec(), None),
    ];
    for id in [
        b"prefix".as_slice(),
        b"duplicate\0ignored bytes",
        b"duplicate-suffix",
        b"e\xff\0x\0",
        b"",
        b"null-codec",
        b"Duplicate",
        b"duplicat",
        b"e",
        b"unknown-encoder-type",
    ] {
        for mask in [0, u32::MAX, 0b1010101] {
            check(&entries, Some(id), mask, true);
        }
        check(&entries, Some(id), 0, false);
    }
    // SAFETY: C owns and mutates all backing storage during each live query.
    assert!(unsafe {
        oracle_encoder_type_info_mutable(obs_encoder_get_display_name, obs_get_encoder_codec)
    });
}
#[test]
fn empty_encoder_registry_does_not_read_query() {
    let unread = core::ptr::dangling::<c_char>();
    let results = [core::ptr::null()];
    // SAFETY: the initialized registry has zero entries, so original C never
    // reads this non-string pointer. Input arrays are unused and the one result is live.
    assert!(unsafe {
        oracle_encoder_type_info_check(
            core::ptr::null(),
            core::ptr::null(),
            0,
            unread,
            results.as_ptr(),
            1,
            0,
            true,
            obs_encoder_get_display_name,
            obs_get_encoder_codec,
        )
    });
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn arbitrary_byte_encoder_type_info(
        entries in prop::collection::vec((prop::collection::vec(any::<u8>(), 0..24), prop::option::of(prop::collection::vec(any::<u8>(), 0..32))), 0..9),
        arbitrary in prop::collection::vec(any::<u8>(), 0..24),
        selected in any::<usize>(), mask in any::<u32>(), names in any::<bool>(),
    ) {
        check(&entries, Some(&arbitrary), mask, names);
        if !entries.is_empty() {
            let selected = selected % entries.len();
            check(&entries, Some(&entries[selected].0), mask, names);
            let mut first_nul = entries[selected].0.clone();
            first_nul.extend_from_slice(b"\0ignored\xff");
            check(&entries, Some(&first_nul), mask, names);
        }
    }
}

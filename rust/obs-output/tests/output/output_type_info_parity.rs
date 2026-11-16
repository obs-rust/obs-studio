//! Tier 3 against original C queries and lookup; no intentional differences.
//! Non-NULL IDs without a core and display-name matches without get_name are C UB.
//! Bounded real-header fixtures, thread-local roots, no test-owned threads.
use core::ffi::c_char;
use obs_output::ffi::output_type_info::{
    Fields, obs_get_output_flags, obs_get_output_supported_audio_codecs,
    obs_get_output_supported_video_codecs, obs_output_get_display_name,
};
use proptest::prelude::*;

type Text = unsafe extern "C" fn(*const c_char) -> *const c_char;
type Flags = unsafe extern "C" fn(*const c_char) -> u32;
unsafe extern "C" {
    fn oracle_output_type_info_check(
        ids: *const *const c_char,
        flags: *const u32,
        videos: *const *const c_char,
        audios: *const *const c_char,
        count: usize,
        query: *const c_char,
        mode: u32,
        name: Text,
        bits: Flags,
        video: Text,
        audio: Text,
    ) -> bool;
    fn oracle_output_type_info_layout(out: *mut usize);
}
type Entry = (Vec<u8>, u32, Option<Vec<u8>>, Option<Vec<u8>>);
fn cstring(bytes: &[u8]) -> Vec<u8> {
    let mut v = bytes.to_vec();
    v.push(0);
    v
}
fn check(entries: &[Entry], query: Option<&[u8]>, mode: u32) {
    let ids: Vec<_> = entries.iter().map(|e| cstring(&e.0)).collect();
    let ids: Vec<_> = ids.iter().map(|s| s.as_ptr().cast()).collect();
    let flags: Vec<_> = entries.iter().map(|e| e.1).collect();
    let videos: Vec<_> = entries
        .iter()
        .map(|e| e.2.as_deref().map(cstring))
        .collect();
    let audios: Vec<_> = entries
        .iter()
        .map(|e| e.3.as_deref().map(cstring))
        .collect();
    let videos: Vec<_> = videos
        .iter()
        .map(|s| s.as_ref().map_or(core::ptr::null(), |s| s.as_ptr().cast()))
        .collect();
    let audios: Vec<_> = audios
        .iter()
        .map(|s| s.as_ref().map_or(core::ptr::null(), |s| s.as_ptr().cast()))
        .collect();
    let query = query.map(cstring);
    // SAFETY: all strings have a trailing NUL (interior NULs are intentional).
    // Arrays remain live through this bounded call; C owns real structs and TLS roots.
    assert!(
        unsafe {
            oracle_output_type_info_check(
                ids.as_ptr(),
                flags.as_ptr(),
                videos.as_ptr(),
                audios.as_ptr(),
                entries.len(),
                query
                    .as_ref()
                    .map_or(core::ptr::null(), |q| q.as_ptr().cast()),
                mode,
                obs_output_get_display_name,
                obs_get_output_flags,
                obs_get_output_supported_video_codecs,
                obs_get_output_supported_audio_codecs,
            )
        },
        "entries={entries:?}, query={query:?}, mode={mode}"
    );
}
fn entry(id: &[u8], flags: u32, video: Option<&[u8]>, audio: Option<&[u8]>) -> Entry {
    (
        id.to_vec(),
        flags,
        video.map(<[u8]>::to_vec),
        audio.map(<[u8]>::to_vec),
    )
}
#[test]
fn characterized_output_type_info_edges() {
    for id in [None, Some(b"".as_slice()), Some(b"unknown-output-type")] {
        check(&[], id, 0);
    }
    for _ in 0..2 {
        check(&[], None, 1);
    }
    let codecs: [Option<&[u8]>; 5] = [
        None,
        Some(b""),
        Some(b"v\xfe;\0x\0"),
        Some(b" H264;;hevc;H264;unknown "),
        Some(b"aac;opus;;aac;"),
    ];
    for flags in (0..32).map(|bit| 1u32 << bit).chain([0, u32::MAX]) {
        for i in 0..5 {
            let entries = [entry(b"known-output", flags, codecs[i], codecs[4 - i])];
            for id in [
                None,
                Some(b"".as_slice()),
                Some(b"unknown-output-type"),
                Some(b"Known-output"),
                Some(b"known-output-suffix"),
                Some(b"known-outpu"),
                Some(b"known-output\0tail"),
            ] {
                check(&entries, id, 0);
            }
            // Metadata remains valid even when the unused get_name is absent.
            check(&entries, Some(b"known-output"), 2);
        }
    }
    let entries = [
        entry(b"prefix", 1, None, None),
        entry(b"duplicate", 0x80000045, Some(b"h264"), Some(b"aac")),
        entry(b"duplicate", 2, Some(b"h264"), Some(b"aac")),
        entry(b"duplicate-suffix", 4, None, None),
        entry(b"o\xff", 8, None, None),
        entry(b"", 16, None, None),
    ];
    for id in [
        Some(b"duplicate\0ignored bytes".as_slice()),
        Some(b"prefix"),
        Some(b"duplicate-suffix"),
        Some(b"o\xff\0x\0"),
        Some(b""),
        Some(b"Duplicate"),
        Some(b"duplicat"),
        Some(b"o"),
        None,
    ] {
        check(&entries, id, 0);
    }
    // A non-string pointer on an empty initialized registry is never read.
    // SAFETY: empty original lookup compares no IDs; all array counts are zero.
    assert!(unsafe {
        oracle_output_type_info_check(
            core::ptr::null(),
            core::ptr::null(),
            core::ptr::null(),
            core::ptr::null(),
            0,
            core::ptr::without_provenance(1),
            0,
            obs_output_get_display_name,
            obs_get_output_flags,
            obs_get_output_supported_video_codecs,
            obs_get_output_supported_audio_codecs,
        )
    });
}
#[test]
fn private_snapshot_matches_real_header() {
    let mut c = [0; 7];
    // SAFETY: the real-header oracle writes seven size_t measurements.
    unsafe { oracle_output_type_info_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            std::mem::offset_of!(Fields, get_name),
            std::mem::offset_of!(Fields, type_data),
            std::mem::offset_of!(Fields, flags),
            std::mem::offset_of!(Fields, video),
            std::mem::offset_of!(Fields, audio)
        ]
    );
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn generated_output_type_info_matches_original_c(
        entries in prop::collection::vec((prop::collection::vec(any::<u8>(), 0..24), any::<u32>(), prop::option::of(prop::collection::vec(any::<u8>(), 0..32)), prop::option::of(prop::collection::vec(any::<u8>(), 0..32))), 0..9),
        query in prop::option::of(prop::collection::vec(any::<u8>(), 0..24)), pick in any::<usize>(), registered in any::<bool>(), omit_name in any::<bool>(),
    ) {
        let query = if registered && !entries.is_empty() { Some(entries[pick % entries.len()].0.as_slice()) } else { query.as_deref() };
        check(&entries, query, if omit_name { 2 } else { 0 });
    }
}

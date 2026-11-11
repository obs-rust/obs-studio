//! Real JSON/filesystem parity with the unmodified C loader.
//! NULL modules are outside the C contract and are not differential inputs.
use obs_core::ffi::module_load_metadata::{ModuleMetadata, obs_module_load_metadata};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::ffi::{CStr, CString, c_char, c_void};
use std::mem::{align_of, offset_of, size_of};
use std::sync::atomic::{AtomicUsize, Ordering};

unsafe extern "C" {
    fn oracle_module_load_metadata_new(path: *mut c_char) -> *mut c_void;
    fn oracle_module_load_metadata_free(module: *mut c_void);
    fn oracle_obs_module_load_metadata(module: *mut c_void) -> i32;
    fn obs_module_load_metadata_slot(module: *mut c_void) -> *mut *mut ModuleMetadata;
    fn oracle_module_load_metadata_layout(values: *mut usize);
}

#[test]
fn real_header_layout() {
    let mut actual = [0; 14];
    // SAFETY: C writes exactly fourteen size_t measurements from obs-internal.h.
    unsafe { oracle_module_load_metadata_layout(actual.as_mut_ptr()) };
    assert_eq!(
        actual,
        [
            size_of::<ModuleMetadata>(),
            align_of::<ModuleMetadata>(),
            offset_of!(ModuleMetadata, display_name),
            offset_of!(ModuleMetadata, version),
            offset_of!(ModuleMetadata, id),
            offset_of!(ModuleMetadata, os_arch),
            offset_of!(ModuleMetadata, description),
            offset_of!(ModuleMetadata, long_description),
            offset_of!(ModuleMetadata, has_icon),
            offset_of!(ModuleMetadata, has_banner),
            offset_of!(ModuleMetadata, repository_url),
            offset_of!(ModuleMetadata, support_url),
            offset_of!(ModuleMetadata, website_url),
            offset_of!(ModuleMetadata, name)
        ]
    );
}

unsafe fn snapshot(module: *mut c_void) -> Option<([Vec<u8>; 10], bool, bool)> {
    // SAFETY: module and each allocated metadata string remain live until fixture free.
    unsafe {
        let m = *obs_module_load_metadata_slot(module);
        if m.is_null() {
            return None;
        }
        let m = &*m;
        let strings = [
            m.display_name,
            m.id,
            m.version,
            m.os_arch,
            m.name,
            m.description,
            m.long_description,
            m.repository_url,
            m.website_url,
            m.support_url,
        ]
        .map(|p| {
            assert!(!p.is_null());
            CStr::from_ptr(p).to_bytes().to_vec()
        });
        Some((strings, m.has_banner, m.has_icon))
    }
}

fn compare(document: Option<&str>) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "obs-manifest-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).unwrap();
    if let Some(document) = document {
        std::fs::write(dir.join("manifest.json"), document).unwrap();
    }
    for suffix in ["", "/"] {
        let path = CString::new(format!("{}{suffix}", dir.to_str().unwrap())).unwrap();
        // SAFETY: both real C fixtures borrow path; both loaders own their output.
        unsafe {
            let c = oracle_module_load_metadata_new(path.as_ptr().cast_mut());
            let r = oracle_module_load_metadata_new(path.as_ptr().cast_mut());
            assert_eq!(oracle_obs_module_load_metadata(c), 0);
            assert_eq!(obs_module_load_metadata(r), 0);
            let expected = snapshot(c);
            let actual = snapshot(r);
            assert_eq!(actual, expected, "document {document:?}");
            assert_eq!(actual.is_some(), document.is_some());
            oracle_module_load_metadata_free(c);
            // Strings were copied, not borrowed from the parser or the C result.
            assert_eq!(snapshot(r), actual);
            oracle_module_load_metadata_free(r);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn manifests_and_typed_defaults() {
    for document in [
        None,
        Some("{}"),
        Some("not json"),
        Some("[]"),
        Some("null"),
        Some(r#"{"has_icon":true}"#),
        Some(r#"{"has_banner":true}"#),
        Some(r#"{"urls":false,"id":12,"has_icon":"yes"}"#),
        Some(
            r#"{"display_name":"Shown","id":"id","version":"1.2","os_arch":"arch","name":"name","description":"short","long_description":"long","urls":{"repository":"repo","website":"web","support":"help"},"has_banner":true,"has_icon":true}"#,
        ),
    ] {
        compare(document);
    }
}

#[test]
fn differential_generated_fields() {
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(64),
        ..Config::default()
    });
    let strategy = (".{0,48}", any::<bool>(), any::<bool>(), any::<u8>());
    runner
        .run(&strategy, |(text, banner, icon, mask)| {
            let mut json = serde_json::json!({});
            for key in [
                "display_name",
                "id",
                "version",
                "os_arch",
                "name",
                "description",
                "long_description",
            ] {
                json[key] = if mask & 1 != 0 {
                    serde_json::json!(text)
                } else {
                    serde_json::json!(17)
                };
            }
            json["urls"] = if mask & 2 != 0 {
                serde_json::json!({"repository":text,"website":text,"support":text})
            } else {
                serde_json::json!(null)
            };
            json["has_banner"] = serde_json::json!(banner);
            json["has_icon"] = serde_json::json!(icon);
            compare(Some(&json.to_string()));
            Ok(())
        })
        .unwrap();
}

//! obs-data through the C ABI against the C oracle (`libobs/obs-data.c`),
//! for the measurements in #1449: key lookup cost by key count (#1450),
//! the JSON round trip (#1452), and a small object filled with defaults the
//! way `obs_get_source_defaults` does it (#1453).
//!
//! `cargo bench -p obs-data --bench data_comparison`

use std::ffi::{CStr, CString, c_char, c_void};
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use obs_c_oracle as _;
use obs_data::ffi::obs_data as rust;
use obs_util as _;

unsafe extern "C" {
    fn oracle_obs_data_create() -> *mut c_void;
    fn oracle_obs_data_create_from_json(json_string: *const c_char) -> *mut c_void;
    fn oracle_obs_data_release(data: *mut c_void);
    fn oracle_obs_data_get_json(data: *mut c_void) -> *const c_char;
    fn oracle_obs_data_set_int(data: *mut c_void, name: *const c_char, val: i64);
    fn oracle_obs_data_get_int(data: *mut c_void, name: *const c_char) -> i64;
    fn oracle_obs_data_set_string(data: *mut c_void, name: *const c_char, val: *const c_char);
    fn oracle_obs_data_set_obj(data: *mut c_void, name: *const c_char, obj: *mut c_void);
    fn oracle_obs_data_set_default_int(data: *mut c_void, name: *const c_char, val: i64);
    fn oracle_obs_data_set_default_bool(data: *mut c_void, name: *const c_char, val: bool);
}

struct Api {
    create: unsafe extern "C" fn() -> *mut c_void,
    from_json: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    release: unsafe extern "C" fn(*mut c_void),
    get_json: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    set_int: unsafe extern "C" fn(*mut c_void, *const c_char, i64),
    get_int: unsafe extern "C" fn(*mut c_void, *const c_char) -> i64,
    set_default_int: unsafe extern "C" fn(*mut c_void, *const c_char, i64),
    set_default_bool: unsafe extern "C" fn(*mut c_void, *const c_char, bool),
}

const RUST: Api = Api {
    create: rust::obs_data_create,
    from_json: rust::obs_data_create_from_json,
    release: rust::obs_data_release,
    get_json: rust::obs_data_get_json,
    set_int: rust::obs_data_set_int,
    get_int: rust::obs_data_get_int,
    set_default_int: rust::obs_data_set_default_int,
    set_default_bool: rust::obs_data_set_default_bool,
};

const C: Api = Api {
    create: oracle_obs_data_create,
    from_json: oracle_obs_data_create_from_json,
    release: oracle_obs_data_release,
    get_json: oracle_obs_data_get_json,
    set_int: oracle_obs_data_set_int,
    get_int: oracle_obs_data_get_int,
    set_default_int: oracle_obs_data_set_default_int,
    set_default_bool: oracle_obs_data_set_default_bool,
};

fn names(n: usize) -> Vec<CString> {
    (0..n)
        .map(|i| CString::new(format!("key_{i}")).unwrap())
        .collect()
}

/// One set and one get of the middle key, on an object with `n` keys.
fn set_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("set_get_int");
    for n in [10, 100, 1_000, 5_000] {
        let keys = names(n);
        let mid = &keys[n / 2];
        for (side, api) in [("rust", &RUST), ("c", &C)] {
            // SAFETY: the API's own objects, released at the end.
            let data = unsafe { (api.create)() };
            for (i, k) in keys.iter().enumerate() {
                // SAFETY: a live object and a NUL-terminated name.
                unsafe { (api.set_int)(data, k.as_ptr(), i as i64) };
            }
            group.bench_with_input(BenchmarkId::new(side, n), &n, |b, _| {
                b.iter(|| {
                    // SAFETY: as above.
                    unsafe {
                        (api.set_int)(data, mid.as_ptr(), 7);
                        black_box((api.get_int)(data, mid.as_ptr()))
                    }
                })
            });
            // SAFETY: the creator's reference.
            unsafe { (api.release)(data) };
        }
    }
    group.finish();
}

/// Create, nine defaults, release: what `obs_get_source_defaults` does for
/// `ffmpeg_source` (#1453). Small objects must not pay for the name index.
fn source_defaults(c: &mut Criterion) {
    let mut group = c.benchmark_group("source_defaults");
    for (side, api) in [("rust", &RUST), ("c", &C)] {
        group.bench_function(side, |b| {
            b.iter(|| {
                // SAFETY: the API's own object, released here; literal names.
                unsafe {
                    let s = (api.create)();
                    (api.set_default_bool)(s, c"is_local_file".as_ptr(), true);
                    (api.set_default_bool)(s, c"looping".as_ptr(), false);
                    (api.set_default_bool)(s, c"clear_on_media_end".as_ptr(), true);
                    (api.set_default_bool)(s, c"restart_on_activate".as_ptr(), true);
                    (api.set_default_bool)(s, c"linear_alpha".as_ptr(), false);
                    (api.set_default_int)(s, c"reconnect_delay_sec".as_ptr(), 10);
                    (api.set_default_int)(s, c"buffering_mb".as_ptr(), 2);
                    (api.set_default_int)(s, c"speed_percent".as_ptr(), 100);
                    (api.set_default_bool)(s, c"log_changes".as_ptr(), true);
                    (api.release)(black_box(s));
                }
            })
        });
    }
    group.finish();
}

/// About 280 KB of JSON: 2,000 child objects with a few fields each.
fn document() -> CString {
    // SAFETY: oracle objects, released here; the JSON text is copied out.
    unsafe {
        let root = oracle_obs_data_create();
        for i in 0..2_000 {
            let child = oracle_obs_data_create();
            for (j, k) in names(5).iter().enumerate() {
                oracle_obs_data_set_int(child, k.as_ptr(), (i * 10 + j) as i64);
            }
            oracle_obs_data_set_string(
                child,
                c"name".as_ptr(),
                c"a settings value of some length".as_ptr(),
            );
            let name = CString::new(format!("source_{i}")).unwrap();
            oracle_obs_data_set_obj(root, name.as_ptr(), child);
            oracle_obs_data_release(child);
        }
        let text = CStr::from_ptr(oracle_obs_data_get_json(root)).to_owned();
        oracle_obs_data_release(root);
        text
    }
}

fn json(c: &mut Criterion) {
    let doc = document();
    let mut group = c.benchmark_group("json");
    group.sample_size(20);
    for (side, api) in [("rust", &RUST), ("c", &C)] {
        group.bench_function(BenchmarkId::new("create_from_json", side), |b| {
            b.iter(|| {
                // SAFETY: a NUL-terminated document; the object is released.
                unsafe {
                    let data = (api.from_json)(doc.as_ptr());
                    (api.release)(black_box(data));
                }
            })
        });
        // SAFETY: as above.
        let data = unsafe { (api.from_json)(doc.as_ptr()) };
        group.bench_function(BenchmarkId::new("get_json", side), |b| {
            // SAFETY: a live object.
            b.iter(|| unsafe { black_box((api.get_json)(data)) })
        });
        // SAFETY: the creator's reference.
        unsafe { (api.release)(data) };
    }
    group.finish();
}

criterion_group!(benches, set_get, source_defaults, json);
criterion_main!(benches);

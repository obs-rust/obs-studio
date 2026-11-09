//! Tier 3: the original C wrapper with recording dependencies vs the safe core.
//! Real reader/BOM handling and the ABI are covered by unchanged Tier 2 C tests.
use super::shader_file::{Event, Fake};
use obs_graphics::shader_file::{Failure, Kind, create};
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int};

#[repr(C)]
struct Trace {
    context: c_int,
    path: c_int,
    readable: c_int,
    reject: c_int,
    output: c_int,
    result: c_int,
    error: c_int,
    reads: c_int,
    vertex: c_int,
    pixel: c_int,
    frees: c_int,
    logs: c_int,
    level: c_int,
    identity: c_int,
    log: [c_char; 256],
}
impl Default for Trace {
    fn default() -> Self {
        Self {
            context: 0,
            path: 0,
            readable: 0,
            reject: 0,
            output: 0,
            result: 0,
            error: 0,
            reads: 0,
            vertex: 0,
            pixel: 0,
            frees: 0,
            logs: 0,
            level: 0,
            identity: 0,
            log: [0; 256],
        }
    }
}
unsafe extern "C" {
    fn oracle_shader_file_run(trace: *mut Trace, pixel: c_int);
}

fn compare(flags: [bool; 7]) {
    let [context, path, readable, reject, output, error, pixel] = flags;
    let mut c = Trace {
        context: context.into(),
        path: path.into(),
        readable: readable.into(),
        reject: reject.into(),
        output: output.into(),
        error: error.into(),
        ..Trace::default()
    };
    // SAFETY: the oracle borrows this initialized trace only for the call.
    unsafe { oracle_shader_file_run(&mut c, pixel.into()) };
    let kind = if pixel { Kind::Pixel } else { Kind::Vertex };
    let mut b = Fake {
        context,
        path,
        source: readable.then(|| Fake::default().source.unwrap()),
        reject,
        output,
        error: if error { 99 } else { 0 },
        events: Vec::new(),
    };
    assert_eq!(create(&mut b, kind) != 0, c.result != 0);
    assert_eq!(b.error != 0, c.error != 0);
    assert_eq!(c.identity, 1);
    let count =
        |predicate: fn(&Event) -> bool| b.events.iter().filter(|e| predicate(e)).count() as c_int;
    assert_eq!(count(|e| matches!(e, Event::Read)), c.reads);
    assert_eq!(
        count(|e| matches!(e, Event::Create(Kind::Vertex, ..))),
        c.vertex
    );
    assert_eq!(
        count(|e| matches!(e, Event::Create(Kind::Pixel, ..))),
        c.pixel
    );
    assert_eq!(count(|e| matches!(e, Event::Free)), c.frees);
    assert_eq!(count(|e| matches!(e, Event::Log(..))), c.logs);
    let name = if pixel {
        "gs_pixelshader_create_from_file"
    } else {
        "gs_vertexshader_create_from_file"
    };
    let mut expected = String::new();
    let mut level = 0;
    for event in &b.events {
        if let Event::Log(_, failure) = event {
            (level, expected) = match failure {
                Failure::Context => (
                    400,
                    format!("{name}: called while not in a graphics context"),
                ),
                Failure::Path => (400, format!("{name}: Null 'file' parameter")),
                Failure::Read => (
                    100,
                    format!(
                        "Could not load {} shader file 'source.shader'",
                        if pixel { "pixel" } else { "vertex" }
                    ),
                ),
            };
        }
    }
    assert_eq!(level, c.level);
    // SAFETY: the oracle's vsnprintf always terminates this bounded buffer.
    assert_eq!(
        unsafe { CStr::from_ptr(c.log.as_ptr()) }.to_str().unwrap(),
        expected
    );
}

#[test]
fn shader_file_all_control_paths_match_original_c() {
    for bits in 0..128 {
        compare(std::array::from_fn(|i| bits & (1 << i) != 0));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn shader_file_randomized_matches_original_c(flags in any::<[bool; 7]>()) { compare(flags); }
}

unsafe extern "C" {
    fn oracle_shader_file_context(present: bool);
    fn oracle_live_vertexshader_create_from_file(
        file: *const c_char,
        error: *mut *mut c_char,
    ) -> *mut std::ffi::c_void;
    fn oracle_live_pixelshader_create_from_file(
        file: *const c_char,
        error: *mut *mut c_char,
    ) -> *mut std::ffi::c_void;
}

fn abi_compare(bytes: &[u8], present: bool, path_present: bool, reject: bool, pixel: bool) {
    use obs_c_oracle::effect_parser_compile as device;
    use obs_graphics::ffi::shader_file as rs;
    use obs_util::ffi::{
        base::{base_get_log_handler, base_set_log_handler},
        darray::bfree,
    };
    use std::{ffi::CString, ptr};
    let _guard = super::LOG_HANDLER.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.shader");
    std::fs::write(&path, bytes).unwrap();
    let path = CString::new(path.to_str().unwrap()).unwrap();
    let path = if path_present {
        path.as_ptr()
    } else {
        ptr::null()
    };
    let mut outcomes = Vec::new();
    // SAFETY: all path/error buffers live across calls, the device returns opaque
    // tags only, and the process-wide recorder is serialized by LOG_HANDLER.
    unsafe {
        let mut handler = ptr::null_mut();
        let mut param = ptr::null_mut();
        base_get_log_handler(&mut handler, &mut param);
        oracle_shader_file_context(present);
        for rust in [false, true] {
            device::oracle_compile_device_reset(reject.into(), reject.into(), 0, 0, ptr::null());
            base_set_log_handler(device::oracle_compile_log_handler_ptr(), ptr::null_mut());
            let mut error = ptr::null_mut();
            let result = match (rust, pixel) {
                (false, false) => oracle_live_vertexshader_create_from_file(path, &mut error),
                (false, true) => oracle_live_pixelshader_create_from_file(path, &mut error),
                (true, false) => rs::gs_vertexshader_create_from_file(path, &mut error),
                (true, true) => rs::gs_pixelshader_create_from_file(path, &mut error),
            };
            let error_text = (!error.is_null()).then(|| CStr::from_ptr(error).to_bytes().to_vec());
            bfree(error.cast());
            let mut len = 0;
            let events = device::oracle_compile_device_events(&mut len);
            let events = std::slice::from_raw_parts(events.cast::<u8>(), len).to_vec();
            outcomes.push((result as usize, error_text, events));
            device::oracle_compile_device_stop();
        }
        oracle_shader_file_context(false);
        base_set_log_handler(handler, param);
    }
    assert_eq!(outcomes[0], outcomes[1]);
}

#[test]
fn shader_file_abi_real_files_match_original_c() {
    for bytes in [
        b"".as_slice(),
        b"\xef\xbb\xbf",
        b"// UTF-8: \xc3\xa9\n",
        b"\xef\xbb\xbfvoid main() {}",
        b"\0tail",
    ] {
        for context in [false, true] {
            for path in [false, true] {
                for reject in [false, true] {
                    for pixel in [false, true] {
                        abi_compare(bytes, context, path, reject, pixel);
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn shader_file_abi_randomized_matches_original_c(bytes in proptest::collection::vec(any::<u8>(), 0..128), present in any::<bool>(), path in any::<bool>(), reject in any::<bool>(), pixel in any::<bool>()) {
        abi_compare(&bytes, present, path, reject, pixel);
    }
}

//! Tier 3: the shims against the original `libobs/graphics/effect-params.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/effect_params.c`) is the C file,
//! unmodified, with its 17 exports renamed. Both sides get a parameter
//! built the same way (type, `cur_val` and `default_val` with the same
//! bytes, `changed`, `next_sampler`) and the same call through their C
//! signatures, and are compared: `cur_val`'s `num`, `capacity` and bytes,
//! `default_val`, `changed`, `next_sampler`, the returned size or copy, and
//! the `LOG_ERROR` lines (count and text, through the process log handler
//! both sides log to).
//!
//! Runs: the `struct gs_shader_texture` layout; chosen cases for every
//! function (sizes up and down, the same bytes, a NULL parameter or data,
//! no default, every parameter type for the next sampler); 20 000 random
//! calls on random parameters.
//!
//! Excluded, the intentional difference: the padding of `struct
//! gs_shader_texture`. For `gs_effect_set_texture` and `_texture_srgb` the
//! C copies and compares its stack padding as it finds it, so only the
//! `tex` and `srgb` fields are compared, and the shim's `changed` must be
//! set whenever the C's would be for the fields alone (the C may set it
//! more often, from the padding).

use core::ffi::{c_char, c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use core::ptr;

use obs_c_oracle::effect_params as c;
use obs_graphics::effect_params::{ByteStore, GS_SHADER_PARAM_TEXTURE};
use obs_graphics::ffi::effect_lookups::gs_effect_param;
use obs_graphics::ffi::effect_params as r;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use obs_util::ffi::darray::darray;
use proptest::prelude::*;

#[repr(C)]
struct Rec {
    calls: c_int,
    level: c_int,
    message: [c_char; 256],
}

impl Rec {
    fn new() -> Self {
        Self {
            calls: 0,
            level: 0,
            message: [0; 256],
        }
    }

    fn text(&self) -> String {
        let bytes: Vec<u8> = self.message.iter().map(|b| *b as u8).collect();
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }
}

unsafe extern "C" {
    fn base_test_record_ptr() -> *mut c_void;
    fn base_test_record_only_this_thread();
    fn base_test_record_any_thread();
    fn bfree(ptr: *mut c_void);
}

/// Runs `f` with the log recorded; returns its result and the record.
fn recorded<T>(f: impl FnOnce() -> T) -> (T, c_int, String) {
    let _guard = super::LOG_HANDLER.lock().unwrap_or_else(|e| e.into_inner());
    let mut rec = Rec::new();
    let (mut handler, mut param) = (ptr::null_mut(), ptr::null_mut());
    // SAFETY: live out-pointers; the record outlives the handler, which is
    // restored before it goes.
    let out = unsafe {
        base_get_log_handler(&mut handler, &mut param);
        base_test_record_only_this_thread();
        base_set_log_handler(base_test_record_ptr(), (&mut rec as *mut Rec).cast());
        let out = f();
        base_set_log_handler(handler, param);
        base_test_record_any_thread();
        out
    };
    let text = if rec.calls > 0 {
        rec.text()
    } else {
        String::new()
    };
    (out, rec.calls, text)
}

#[test]
fn layout() {
    let values = [
        size_of::<r::gs_shader_texture>(),
        align_of::<r::gs_shader_texture>(),
        offset_of!(r::gs_shader_texture, tex),
        offset_of!(r::gs_shader_texture, srgb),
    ];
    for (which, (name, value)) in c::LAYOUT_NAMES.iter().zip(values).enumerate() {
        // SAFETY: a pure C function.
        assert_eq!(
            unsafe { c::oracle_effect_params_layout(which) },
            value,
            "{name}"
        );
    }
}

/// A parameter as both sides start a call.
#[derive(Debug, Clone)]
struct Start {
    param_type: i32,
    cur: Vec<u8>,
    /// Extra capacity of `cur_val` beyond its bytes.
    cur_slack: usize,
    default: Option<Vec<u8>>,
    changed: bool,
    sampler: usize,
}

/// One call.
#[derive(Debug, Clone)]
enum Call {
    Bool(bool),
    Float(f32),
    Int(i32),
    Matrix4([f32; 16]),
    Vec2([f32; 2]),
    Vec3([f32; 4]),
    Vec4([f32; 4]),
    Color(u32),
    Texture(usize, bool),
    Val(Option<Vec<u8>>),
    NullData(u8),
    GetVal,
    GetValSize,
    GetDefault,
    GetDefaultSize,
    SetDefault,
    NextSampler(usize),
}

/// What a side ends with.
#[derive(Debug, PartialEq)]
struct End {
    cur_num: usize,
    cur_cap: usize,
    cur: Vec<u8>,
    default: Vec<u8>,
    changed: bool,
    sampler: usize,
    ret: Option<Vec<u8>>,
    size: Option<usize>,
    log_calls: c_int,
    log_text: String,
}

fn empty_darray() -> darray {
    darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    }
}

/// The parameter, built the same way for each side: the darrays through
/// the shim's store (bmem memory, C growth rules).
fn build(start: &Start) -> Box<gs_effect_param> {
    let mut p = Box::new(gs_effect_param {
        name: ptr::null_mut(),
        section: 0,
        type_: start.param_type,
        changed: start.changed,
        cur_val: empty_darray(),
        default_val: empty_darray(),
        effect: ptr::null_mut(),
        next_sampler: start.sampler as *mut c_void,
        annotations: empty_darray(),
    });
    // SAFETY: the parameter's own darrays.
    unsafe {
        if start.cur_slack > 0 {
            let mut s = r::DarrayStore::new(&mut p.cur_val);
            s.resize(start.cur.len() + start.cur_slack);
            s.resize(start.cur.len());
        }
        let mut s = r::DarrayStore::new(&mut p.cur_val);
        s.resize(start.cur.len());
        s.bytes_mut().copy_from_slice(&start.cur);
        if let Some(d) = &start.default {
            let mut s = r::DarrayStore::new(&mut p.default_val);
            s.resize(d.len());
            s.bytes_mut().copy_from_slice(d);
        }
    }
    p
}

fn bytes_of(da: &darray) -> Vec<u8> {
    if da.num == 0 || da.array.is_null() {
        return Vec::new();
    }
    // SAFETY: `num` bytes at `array`.
    unsafe { core::slice::from_raw_parts(da.array.cast::<u8>(), da.num).to_vec() }
}

fn take_copy(p: *mut c_void, size: usize) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: a bmem copy of `size` bytes, freed here.
    unsafe {
        let v = core::slice::from_raw_parts(p.cast::<u8>(), size).to_vec();
        bfree(p);
        Some(v)
    }
}

fn floats(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

/// Runs `call` on one side. `null_param` passes NULL instead.
#[expect(
    clippy::too_many_lines,
    reason = "parity table: one block per ported C case"
)]
fn run(start: &Start, call: &Call, rust: bool, null_param: bool) -> End {
    let mut p = build(start);
    let pp: *mut gs_effect_param = if null_param { ptr::null_mut() } else { &mut *p };
    let vp = pp.cast::<c_void>();
    let ((ret, size), log_calls, log_text) = recorded(|| {
        // SAFETY: a valid or null parameter; the data buffers live for the
        // call and are at least the size each setter reads.
        unsafe {
            let mut ret = None;
            let mut size = None;
            match call {
                Call::Bool(b) => {
                    if rust {
                        r::gs_effect_set_bool(pp, *b)
                    } else {
                        c::oracle_gs_effect_set_bool(vp, *b)
                    }
                }
                Call::Float(f) => {
                    if rust {
                        r::gs_effect_set_float(pp, *f)
                    } else {
                        c::oracle_gs_effect_set_float(vp, *f)
                    }
                }
                Call::Int(i) => {
                    if rust {
                        r::gs_effect_set_int(pp, *i)
                    } else {
                        c::oracle_gs_effect_set_int(vp, *i)
                    }
                }
                Call::Matrix4(m) => {
                    let b = floats(m);
                    if rust {
                        r::gs_effect_set_matrix4(pp, b.as_ptr().cast())
                    } else {
                        c::oracle_gs_effect_set_matrix4(vp, b.as_ptr().cast())
                    }
                }
                Call::Vec2(v) => {
                    let b = floats(v);
                    if rust {
                        r::gs_effect_set_vec2(pp, b.as_ptr().cast())
                    } else {
                        c::oracle_gs_effect_set_vec2(vp, b.as_ptr().cast())
                    }
                }
                Call::Vec3(v) => {
                    let b = floats(v);
                    if rust {
                        r::gs_effect_set_vec3(pp, b.as_ptr().cast())
                    } else {
                        c::oracle_gs_effect_set_vec3(vp, b.as_ptr().cast())
                    }
                }
                Call::Vec4(v) => {
                    let b = floats(v);
                    if rust {
                        r::gs_effect_set_vec4(pp, b.as_ptr().cast())
                    } else {
                        c::oracle_gs_effect_set_vec4(vp, b.as_ptr().cast())
                    }
                }
                Call::Color(argb) => {
                    if rust {
                        r::gs_effect_set_color(pp, *argb)
                    } else {
                        c::oracle_gs_effect_set_color(vp, *argb)
                    }
                }
                Call::Texture(t, srgb) => {
                    let t = *t as *mut c_void;
                    match (rust, srgb) {
                        (true, false) => r::gs_effect_set_texture(pp, t),
                        (true, true) => r::gs_effect_set_texture_srgb(pp, t),
                        (false, false) => c::oracle_gs_effect_set_texture(vp, t),
                        (false, true) => c::oracle_gs_effect_set_texture_srgb(vp, t),
                    }
                }
                Call::Val(data) => {
                    let (dp, n) = data
                        .as_ref()
                        .map_or((ptr::null(), 0), |d| (d.as_ptr().cast::<c_void>(), d.len()));
                    if rust {
                        r::gs_effect_set_val(pp, dp, n)
                    } else {
                        c::oracle_gs_effect_set_val(vp, dp, n)
                    }
                }
                Call::NullData(which) => {
                    let n = ptr::null();
                    match (rust, which % 4) {
                        (true, 0) => r::gs_effect_set_vec2(pp, n),
                        (true, 1) => r::gs_effect_set_vec3(pp, n),
                        (true, 2) => r::gs_effect_set_vec4(pp, n),
                        (true, _) => r::gs_effect_set_matrix4(pp, n),
                        (false, 0) => c::oracle_gs_effect_set_vec2(vp, n),
                        (false, 1) => c::oracle_gs_effect_set_vec3(vp, n),
                        (false, 2) => c::oracle_gs_effect_set_vec4(vp, n),
                        (false, _) => c::oracle_gs_effect_set_matrix4(vp, n),
                    }
                }
                Call::GetVal => {
                    let n = if null_param { 0 } else { p.cur_val.num };
                    let out = if rust {
                        r::gs_effect_get_val(pp)
                    } else {
                        c::oracle_gs_effect_get_val(vp)
                    };
                    ret = Some(take_copy(out, n).unwrap_or_default());
                }
                Call::GetDefault => {
                    let n = if null_param { 0 } else { p.default_val.num };
                    let out = if rust {
                        r::gs_effect_get_default_val(pp)
                    } else {
                        c::oracle_gs_effect_get_default_val(vp)
                    };
                    ret = Some(take_copy(out, n).unwrap_or_default());
                }
                Call::GetValSize => {
                    size = Some(if rust {
                        r::gs_effect_get_val_size(pp)
                    } else {
                        c::oracle_gs_effect_get_val_size(vp)
                    });
                }
                Call::GetDefaultSize => {
                    size = Some(if rust {
                        r::gs_effect_get_default_val_size(pp)
                    } else {
                        c::oracle_gs_effect_get_default_val_size(vp)
                    });
                }
                Call::SetDefault => {
                    if rust {
                        r::gs_effect_set_default(pp)
                    } else {
                        c::oracle_gs_effect_set_default(vp)
                    }
                }
                Call::NextSampler(s) => {
                    let s = *s as *mut c_void;
                    if rust {
                        r::gs_effect_set_next_sampler(pp, s)
                    } else {
                        c::oracle_gs_effect_set_next_sampler(vp, s)
                    }
                }
            }
            (ret, size)
        }
    });
    let end = End {
        cur_num: p.cur_val.num,
        cur_cap: p.cur_val.capacity,
        cur: bytes_of(&p.cur_val),
        default: bytes_of(&p.default_val),
        changed: p.changed,
        sampler: p.next_sampler as usize,
        ret,
        size,
        log_calls,
        log_text,
    };
    // SAFETY: bmem darrays of this parameter.
    unsafe {
        bfree(p.cur_val.array);
        bfree(p.default_val.array);
    }
    end
}

const TEX_SIZE: usize = size_of::<r::gs_shader_texture>();
const SRGB_AT: usize = offset_of!(r::gs_shader_texture, srgb);

/// `tex` and `srgb` of a stored `struct gs_shader_texture`.
fn texture_fields(bytes: &[u8]) -> (Vec<u8>, u8) {
    (bytes[..size_of::<*mut c_void>()].to_vec(), bytes[SRGB_AT])
}

fn compare(start: &Start, call: &Call, null_param: bool) {
    let rs = run(start, call, true, null_param);
    let cs = run(start, call, false, null_param);
    if let (Call::Texture(..), false) = (call, null_param) {
        assert_eq!(
            (
                rs.cur_num,
                rs.cur_cap,
                texture_fields(&rs.cur),
                rs.log_calls
            ),
            (
                cs.cur_num,
                cs.cur_cap,
                texture_fields(&cs.cur),
                cs.log_calls
            ),
            "{start:?} {call:?}"
        );
        assert_eq!(rs.cur_num, TEX_SIZE);
        // The shim's padding is zero.
        assert!(
            rs.cur
                .iter()
                .enumerate()
                .all(|(i, b)| i < size_of::<*mut c_void>() || i == SRGB_AT || *b == 0)
        );
        // The shim marks a change whenever the fields or size changed.
        let fields_changed =
            start.cur.len() != TEX_SIZE || texture_fields(&start.cur) != texture_fields(&rs.cur);
        assert_eq!(
            rs.changed,
            start.changed || start.cur[..] != rs.cur[..],
            "{start:?} {call:?}"
        );
        if fields_changed {
            assert!(cs.changed, "{start:?} {call:?}");
        }
    } else {
        assert_eq!(rs, cs, "{start:?} {call:?} null_param={null_param}");
    }
}

fn start(param_type: i32, cur: &[u8], default: Option<&[u8]>) -> Start {
    Start {
        param_type,
        cur: cur.to_vec(),
        cur_slack: 0,
        default: default.map(<[u8]>::to_vec),
        changed: false,
        sampler: 0,
    }
}

#[test]
fn chosen_cases() {
    let f4 = 1.5f32.to_ne_bytes();
    let v16 = floats(&[1.0, 2.0, 3.0, 4.0]);
    let calls = [
        Call::Bool(true),
        Call::Bool(false),
        Call::Float(1.5),
        Call::Float(2.0),
        Call::Int(-7),
        Call::Matrix4([0.5; 16]),
        Call::Vec2([1.0, 2.0]),
        Call::Vec3([1.0, 2.0, 3.0, 99.0]),
        Call::Vec4([1.0, 2.0, 3.0, 4.0]),
        Call::Color(0x80FF_4020),
        Call::Color(0),
        Call::Texture(0x1234, false),
        Call::Texture(0x1234, true),
        Call::Val(Some(vec![1, 2, 3])),
        Call::Val(Some(Vec::new())),
        Call::Val(None),
        Call::NullData(0),
        Call::NullData(1),
        Call::NullData(2),
        Call::NullData(3),
        Call::GetVal,
        Call::GetValSize,
        Call::GetDefault,
        Call::GetDefaultSize,
        Call::SetDefault,
        Call::NextSampler(0x5678),
    ];
    let starts = [
        start(2, &[], None),
        start(2, &f4, None),
        start(7, &v16, Some(&floats(&[5.0, 6.0, 7.0, 8.0]))),
        start(7, &v16, Some(&[])),
        start(GS_SHADER_PARAM_TEXTURE, &[], None),
        Start {
            changed: true,
            sampler: 0x9,
            cur_slack: 40,
            ..start(GS_SHADER_PARAM_TEXTURE, &[0u8; 16], Some(&v16))
        },
    ];
    for s in &starts {
        for call in &calls {
            compare(s, call, false);
            compare(s, call, true);
        }
    }
    // The next sampler on every type.
    for t in 0..=GS_SHADER_PARAM_TEXTURE {
        compare(&start(t, &[], None), &Call::NextSampler(0x5678), false);
    }
}

fn call_strategy() -> impl Strategy<Value = Call> {
    let f = prop_oneof![
        any::<f32>().prop_filter("not NaN", |x| !x.is_nan()),
        Just(0.0f32),
        Just(1.0f32)
    ];
    prop_oneof![
        any::<bool>().prop_map(Call::Bool),
        f.clone().prop_map(Call::Float),
        any::<i32>().prop_map(Call::Int),
        prop::array::uniform16(f.clone()).prop_map(Call::Matrix4),
        prop::array::uniform2(f.clone()).prop_map(Call::Vec2),
        prop::array::uniform4(f.clone()).prop_map(Call::Vec3),
        prop::array::uniform4(f).prop_map(Call::Vec4),
        any::<u32>().prop_map(Call::Color),
        (any::<usize>(), any::<bool>()).prop_map(|(t, s)| Call::Texture(t, s)),
        prop::option::of(prop::collection::vec(any::<u8>(), 0..96)).prop_map(Call::Val),
        any::<u8>().prop_map(Call::NullData),
        Just(Call::GetVal),
        Just(Call::GetValSize),
        Just(Call::GetDefault),
        Just(Call::GetDefaultSize),
        Just(Call::SetDefault),
        any::<usize>().prop_map(Call::NextSampler),
    ]
}

fn start_strategy() -> impl Strategy<Value = Start> {
    (
        0i32..=GS_SHADER_PARAM_TEXTURE,
        prop::collection::vec(any::<u8>(), 0..96),
        0usize..64,
        prop::option::of(prop::collection::vec(any::<u8>(), 0..96)),
        any::<bool>(),
        any::<usize>(),
    )
        .prop_map(
            |(param_type, cur, cur_slack, default, changed, sampler)| Start {
                param_type,
                cur,
                cur_slack,
                default,
                changed,
                sampler,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn random_calls_match(s in start_strategy(), call in call_strategy(), null_param in prop::bool::weighted(0.05)) {
        compare(&s, &call, null_param);
    }
}

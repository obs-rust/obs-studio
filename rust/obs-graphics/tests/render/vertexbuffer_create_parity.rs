//! Tier 2 layouts and Tier 3 original-C parity. Inputs bound vertices to eight
//! and textures to four of width one through four. Zero-width textures with
//! nonzero vertices violate the original bmalloc(0) fatal contract and are
//! excluded. No intentional differences.
use obs_c_oracle as _;
use obs_graphics::ffi::vertexbuffer_create::{
    TextureArray, VertexBufferData, gs_vertexbuffer_create,
};
use obs_graphics::vertexbuffer_create::DUP_BUFFER;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};
use std::mem::{align_of, offset_of, size_of};

unsafe extern "C" {
    fn oracle_gs_vertexbuffer_create(data: *mut VertexBufferData, flags: u32) -> *mut c_void;
    fn oracle_vertexbuffer_create_reset(present: bool, reject: bool);
    fn oracle_vertexbuffer_create_calls() -> usize;
    fn oracle_vertexbuffer_create_input() -> *mut VertexBufferData;
    fn oracle_vertexbuffer_create_flags() -> u32;
    fn oracle_vertexbuffer_create_device_matches() -> bool;
    fn oracle_vertexbuffer_create_result() -> *mut c_void;
    fn oracle_vertexbuffer_create_logs() -> u32;
    fn oracle_vertexbuffer_create_level() -> i32;
    fn oracle_vertexbuffer_create_log() -> *const c_char;
    fn oracle_vertexbuffer_create_free(data: *mut VertexBufferData);
    fn oracle_vertexbuffer_create_layout(key: usize) -> usize;
}

#[test]
fn public_layouts_match_c_header() {
    let values = [
        size_of::<VertexBufferData>(),
        align_of::<VertexBufferData>(),
        offset_of!(VertexBufferData, num),
        offset_of!(VertexBufferData, points),
        offset_of!(VertexBufferData, normals),
        offset_of!(VertexBufferData, tangents),
        offset_of!(VertexBufferData, colors),
        offset_of!(VertexBufferData, num_tex),
        offset_of!(VertexBufferData, tvarray),
        size_of::<TextureArray>(),
        align_of::<TextureArray>(),
        offset_of!(TextureArray, width),
        offset_of!(TextureArray, array),
        16,
        size_of::<f32>(),
    ];
    for (key, value) in values.into_iter().enumerate() {
        // SAFETY: the key is within the C measurement table.
        assert_eq!(
            value,
            unsafe { oracle_vertexbuffer_create_layout(key) },
            "layout key {key}"
        );
    }
}

#[derive(Clone, Debug)]
struct Case {
    context: bool,
    present: bool,
    reject: bool,
    num: usize,
    mask: u8,
    flags: u32,
    texture_pointer: bool,
    widths: Vec<usize>,
    seed: u8,
}

#[derive(Debug, PartialEq)]
struct Observed {
    calls: usize,
    flags: u32,
    device: bool,
    copied: bool,
    num: usize,
    num_tex: usize,
    texture_pointer: bool,
    arrays: Vec<Option<Vec<u8>>>,
    textures: Vec<(usize, Vec<u8>)>,
    logs: u32,
    level: i32,
    log: Vec<u8>,
}

fn bytes(pointer: *mut c_void, size: usize) -> Vec<u8> {
    if size == 0 {
        return vec![];
    }
    // SAFETY: only called with fixture storage or the implementation's copy,
    // both readable for the public count/width-derived size.
    unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), size).to_vec() }
}

fn check_copy(copy: *mut c_void, source: *mut c_void, expected: &mut [u8]) {
    assert_ne!(copy, source);
    assert_eq!(bytes(copy, expected.len()), expected);
    if expected.is_empty() {
        return;
    }
    // SAFETY: fixture and duplicate are independent writable allocations of
    // expected.len() bytes. Restore each byte before the fixture is reused.
    unsafe {
        let original = expected[0];
        *copy.cast::<u8>() ^= 0xff;
        assert_eq!(expected[0], original);
        *copy.cast::<u8>() ^= 0xff;
        expected[0] ^= 0xff;
        assert_eq!(*copy.cast::<u8>(), original);
        expected[0] = original;
    }
}

fn observe_data(
    data: &VertexBufferData,
    input: &VertexBufferData,
    arrays: &mut [Vec<u8>],
    observed: &mut Observed,
) {
    observed.num = data.num;
    observed.num_tex = data.num_tex;
    observed.texture_pointer = !data.tvarray.is_null();
    for (i, (ptr, width)) in [data.points, data.normals, data.tangents, data.colors]
        .into_iter()
        .zip([16, 16, 16, 4])
        .enumerate()
    {
        observed
            .arrays
            .push((!ptr.is_null()).then(|| bytes(ptr, width * data.num)));
        if observed.copied && !ptr.is_null() {
            check_copy(ptr, arrays[i].as_mut_ptr().cast(), &mut arrays[i]);
        }
    }
    if !data.tvarray.is_null() && data.num_tex != 0 {
        if observed.copied {
            assert_ne!(data.tvarray, input.tvarray);
        }
        // SAFETY: the public array count describes live fixture or copied
        // metadata. Each coordinate allocation has width * num floats.
        for (i, tv) in unsafe { std::slice::from_raw_parts(data.tvarray, data.num_tex) }
            .iter()
            .enumerate()
        {
            observed
                .textures
                .push((tv.width, bytes(tv.array, tv.width * 4 * data.num)));
            if observed.copied {
                check_copy(
                    tv.array,
                    arrays[4 + i].as_mut_ptr().cast(),
                    &mut arrays[4 + i],
                );
            }
        }
    }
}

fn check_dispatch(observed: &Observed, case: &Case) {
    assert_eq!(observed.calls, usize::from(case.context));
    if case.context {
        assert!(observed.device);
        assert_eq!(observed.flags, case.flags);
        assert_eq!(observed.logs, 0);
    } else {
        assert_eq!((observed.logs, observed.level), (1, 400));
        assert_eq!(
            observed.log,
            b"gs_vertexbuffer_create: called while not in a graphics context"
        );
    }
}

fn run(rust: bool, case: &Case) -> Observed {
    let mut arrays: Vec<Vec<u8>> = [16, 16, 16, 4]
        .into_iter()
        .chain(case.widths.iter().map(|w| w * 4))
        .enumerate()
        .map(|(i, width)| {
            (0..width * case.num)
                .map(|j| case.seed.wrapping_add(i as u8).wrapping_add(j as u8))
                .collect()
        })
        .collect();
    let mut textures: Vec<_> = case
        .widths
        .iter()
        .enumerate()
        .map(|(i, &width)| TextureArray {
            width,
            array: arrays[4 + i].as_mut_ptr().cast(),
        })
        .collect();
    let [points, normals, tangents, colors] = std::array::from_fn(|i| {
        if case.mask & (1 << i) != 0 {
            arrays[i].as_mut_ptr().cast()
        } else {
            std::ptr::null_mut()
        }
    });
    let mut input = VertexBufferData {
        num: case.num,
        points,
        normals,
        tangents,
        colors,
        num_tex: textures.len(),
        tvarray: if case.texture_pointer {
            textures.as_mut_ptr()
        } else {
            std::ptr::null_mut()
        },
    };
    let pointer = if case.present {
        &mut input
    } else {
        std::ptr::null_mut()
    };
    // SAFETY: fixture arrays remain live throughout the call, and thread-local
    // oracle state isolates parallel tests. The fake backend retains ownership
    // on failure too; copied data is destroyed using graphics.h's real helper.
    unsafe {
        oracle_vertexbuffer_create_reset(case.context, case.reject);
        let result = if rust {
            gs_vertexbuffer_create(pointer, case.flags)
        } else {
            oracle_gs_vertexbuffer_create(pointer, case.flags)
        };
        assert_eq!(
            result,
            if case.context {
                oracle_vertexbuffer_create_result()
            } else {
                std::ptr::null_mut()
            }
        );
        let transferred = oracle_vertexbuffer_create_input();
        let copied = !transferred.is_null() && transferred != pointer;
        assert_eq!(
            copied,
            case.context && case.present && case.num != 0 && case.flags & DUP_BUFFER != 0
        );
        let mut observed = Observed {
            calls: oracle_vertexbuffer_create_calls(),
            flags: oracle_vertexbuffer_create_flags(),
            device: oracle_vertexbuffer_create_device_matches(),
            copied,
            num: 0,
            num_tex: 0,
            texture_pointer: false,
            arrays: vec![],
            textures: vec![],
            logs: oracle_vertexbuffer_create_logs(),
            level: oracle_vertexbuffer_create_level(),
            log: CStr::from_ptr(oracle_vertexbuffer_create_log())
                .to_bytes()
                .to_vec(),
        };
        check_dispatch(&observed, case);
        if let Some(data) = transferred.as_ref() {
            observe_data(data, &input, &mut arrays, &mut observed);
        }
        if copied {
            oracle_vertexbuffer_create_free(transferred);
        }
        assert_eq!(input.num, case.num);
        assert_eq!(input.num_tex, textures.len());
        observed
    }
}

fn compare(case: &Case) {
    assert_eq!(run(true, case), run(false, case), "{case:?}");
}

#[test]
fn characterized_ownership_matrix_matches_c() {
    for context in [false, true] {
        for present in [false, true] {
            for reject in [false, true] {
                for num in [0, 3] {
                    for mask in 0..16 {
                        for flags in [0, 2, DUP_BUFFER, DUP_BUFFER | 2, u32::MAX] {
                            for (texture_pointer, widths) in [
                                (false, vec![1, 2, 4]),
                                (true, vec![]),
                                (true, vec![1, 2, 4]),
                            ] {
                                compare(&Case {
                                    context,
                                    present,
                                    reject,
                                    num,
                                    mask,
                                    flags,
                                    texture_pointer,
                                    widths,
                                    seed: 0x13,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_ownership_matches_c(
        state in (any::<bool>(), any::<bool>(), any::<bool>()),
        num in 0usize..=8, mask in 0u8..16, flags in any::<u32>(),
        texture_pointer in any::<bool>(), widths in prop::collection::vec(1usize..=4, 0..=4), seed in any::<u8>(),
    ) {
        compare(&Case { context: state.0, present: state.1, reject: state.2, num, mask, flags, texture_pointer, widths, seed });
    }
}

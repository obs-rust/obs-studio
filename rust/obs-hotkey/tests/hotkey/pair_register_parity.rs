//! Tier 3: the original C pair registration wrappers vs the public Rust ABI,
//! through the same real-layout adapters and a recording pair engine that
//! calls the weak helper once per half. No intentional differences. State is
//! thread-local in the C oracle.
use super::pair_register::KINDS;
use core::ffi::{c_char, c_int, c_void};
use obs_hotkey::ffi::pair_register::{self, ActiveCallback};
use proptest::prelude::*;

type Entry = unsafe extern "C" fn(
    *mut c_void,
    *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
    ActiveCallback,
    ActiveCallback,
    *mut c_void,
    *mut c_void,
) -> usize;

unsafe extern "C" {
    fn oracle_obs_hotkey_pair_register_encoder(
        owner: *mut c_void,
        name0: *const c_char,
        description0: *const c_char,
        name1: *const c_char,
        description1: *const c_char,
        func0: ActiveCallback,
        func1: ActiveCallback,
        data0: *mut c_void,
        data1: *mut c_void,
    ) -> usize;
    fn oracle_obs_hotkey_pair_register_output(
        owner: *mut c_void,
        name0: *const c_char,
        description0: *const c_char,
        name1: *const c_char,
        description1: *const c_char,
        func0: ActiveCallback,
        func1: ActiveCallback,
        data0: *mut c_void,
        data1: *mut c_void,
    ) -> usize;
    fn oracle_obs_hotkey_pair_register_service(
        owner: *mut c_void,
        name0: *const c_char,
        description0: *const c_char,
        name1: *const c_char,
        description1: *const c_char,
        func0: ActiveCallback,
        func1: ActiveCallback,
        data0: *mut c_void,
        data1: *mut c_void,
    ) -> usize;
    fn oracle_obs_hotkey_pair_register_source(
        owner: *mut c_void,
        name0: *const c_char,
        description0: *const c_char,
        name1: *const c_char,
        description1: *const c_char,
        func0: ActiveCallback,
        func1: ActiveCallback,
        data0: *mut c_void,
        data1: *mut c_void,
    ) -> usize;
    fn oracle_hotkey_pair_register_reset(
        kind: c_int,
        result: usize,
        func0: ActiveCallback,
        func1: ActiveCallback,
    ) -> *mut c_void;
    fn oracle_hotkey_pair_register_snapshot(trace: *mut usize) -> usize;
    fn oracle_hotkey_pair_register_offsets(values: *mut usize);
}

unsafe extern "C" fn first(_: *mut c_void, _: usize, _: *mut c_void, _: bool) -> bool {
    false
}
unsafe extern "C" fn second(_: *mut c_void, _: usize, _: *mut c_void, _: bool) -> bool {
    true
}

/// Entry points in `KINDS` order: encoder, output, service, source.
const RUST: [Entry; 4] = [
    pair_register::obs_hotkey_pair_register_encoder,
    pair_register::obs_hotkey_pair_register_output,
    pair_register::obs_hotkey_pair_register_service,
    pair_register::obs_hotkey_pair_register_source,
];
const ORIGINAL: [Entry; 4] = [
    oracle_obs_hotkey_pair_register_encoder,
    oracle_obs_hotkey_pair_register_output,
    oracle_obs_hotkey_pair_register_service,
    oracle_obs_hotkey_pair_register_source,
];

#[derive(Clone, Debug)]
struct Input {
    kind: usize,
    present: bool,
    result: usize,
    /// NULL name0, description0, name1, description1, func0, func1, data0, data1.
    nulls: [bool; 8],
    /// Swap the two callbacks, so each half's function stays distinct.
    swap: bool,
}

fn run(input: &Input, rust: bool) -> (usize, Vec<usize>) {
    let kind = KINDS[input.kind];
    // SAFETY: resets only this thread's oracle fixture; the returned owner
    // has the real layout of the selected kind.
    let owner = unsafe {
        oracle_hotkey_pair_register_reset(kind.code(), input.result, Some(first), Some(second))
    };
    let owner = if input.present {
        owner
    } else {
        core::ptr::null_mut()
    };
    let text = [
        c"object.on".as_ptr(),
        c"Enable object".as_ptr(),
        c"object.off".as_ptr(),
        c"Disable object".as_ptr(),
    ];
    let [name0, description0, name1, description1] = core::array::from_fn(|i| {
        if input.nulls[i] {
            core::ptr::null()
        } else {
            text[i]
        }
    });
    let (a, b): (ActiveCallback, ActiveCallback) = if input.swap {
        (Some(second), Some(first))
    } else {
        (Some(first), Some(second))
    };
    let func0 = if input.nulls[4] { None } else { a };
    let func1 = if input.nulls[5] { None } else { b };
    let data0 = if input.nulls[6] {
        core::ptr::null_mut()
    } else {
        text[0].cast_mut().cast::<c_void>()
    };
    let data1 = if input.nulls[7] {
        core::ptr::null_mut()
    } else {
        text[2].cast_mut().cast::<c_void>()
    };
    let entry = if rust { RUST } else { ORIGINAL }[input.kind];
    // SAFETY: owners come from the real C fixture, and the recording engine
    // borrows the arguments without dereferencing strings, callbacks or data.
    let result = unsafe {
        entry(
            owner,
            name0,
            description0,
            name1,
            description1,
            func0,
            func1,
            data0,
            data1,
        )
    };
    let mut trace = [0; 64];
    // SAFETY: the buffer has the C helper's exact capacity.
    let count = unsafe { oracle_hotkey_pair_register_snapshot(trace.as_mut_ptr()) };
    (result, trace[..count].to_vec())
}

#[test]
fn real_header_layout_and_values() {
    let mut values = [0; 6];
    // SAFETY: the helper writes six size_t values to this exact-size array.
    unsafe { oracle_hotkey_pair_register_offsets(values.as_mut_ptr()) };
    assert_eq!(&values[4..], [size_of::<usize>(), usize::MAX]);
    assert_eq!(
        KINDS.map(|kind| kind.code()),
        [3, 2, 4, 1],
        "encoder, output, service, source registerer kinds"
    );
    // The engine receives each owner's own embedded context.
    for (index, offset) in [3, 1, 0, 2].into_iter().zip(values) {
        let input = Input {
            kind: index,
            present: true,
            result: 5,
            nulls: [false; 8],
            swap: false,
        };
        let (result, trace) = run(&input, true);
        assert_eq!(result, 5);
        // SAFETY: only reads this thread's fixture address for the kind.
        let owner =
            unsafe { oracle_hotkey_pair_register_reset(KINDS[index].code(), 0, None, None) };
        assert_eq!(trace[2], owner as usize);
        assert_eq!(trace[3], owner as usize + offset);
    }
}

#[test]
fn characterized_admission_and_full_width_results() {
    for kind in 0..4 {
        for present in [false, true] {
            for result in [0, 1, usize::MAX - 1, usize::MAX] {
                let input = Input {
                    kind,
                    present,
                    result,
                    nulls: [false; 8],
                    swap: false,
                };
                let rust = run(&input, true);
                assert_eq!(rust, run(&input, false), "{input:?}");
                if present {
                    // One engine call (12 entries); then, once per half, the
                    // weak helper runs on the owner itself (4 entries each).
                    let trace = &rust.1;
                    assert_eq!(trace.len(), 20);
                    assert_eq!([trace[0], trace[12], trace[16]], [3, 4, 4]);
                    assert_eq!([trace[14], trace[18]], [1, 1]);
                    assert_eq!(trace[15], trace[19]);
                } else {
                    assert_eq!(rust, (usize::MAX, Vec::new()));
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn wrappers_match_original_c(
        kind in 0usize..4,
        present in any::<bool>(),
        result in any::<usize>(),
        nulls in any::<[bool; 8]>(),
        swap in any::<bool>(),
    ) {
        let input = Input { kind, present, result, nulls, swap };
        prop_assert_eq!(run(&input, true), run(&input, false));
    }
}

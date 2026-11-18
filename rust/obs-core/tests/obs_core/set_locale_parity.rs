//! Tier 3: original C vs actual Rust FFI, using real field adapters and bmem.
//! NULL core and input aliasing old storage are outside the characterized C
//! domain. No intentional behavior differences. No new test threads.
use obs_core::ffi::set_locale::obs_set_locale;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::ffi::{CString, c_char};

unsafe extern "C" {
    fn oracle_set_locale_reset(order: *const u8, count: u32, callbacks: u32, mutate: bool);
    fn oracle_set_locale_step(rust: bool, locale: *mut c_char, out: *mut u64) -> usize;
    fn oracle_set_locale_finish();
    fn oracle_set_locale_layout(out: *mut usize);
}
fn run(
    rust: bool,
    order: &[u8; 6],
    count: u32,
    callbacks: u32,
    mutate: bool,
    steps: &[Option<Vec<u8>>],
) -> Vec<Vec<u64>> {
    // Reference the real shim explicitly as well as through the C fixture.
    let _entry = obs_set_locale;
    // SAFETY: order contains six unique valid indices; count <= 6. Each input
    // has at most 64 non-NUL bytes and is mutable, live, and independent of the
    // old locale. The fixture writes <= 114 u64s and frees its owned locale.
    unsafe {
        oracle_set_locale_reset(order.as_ptr(), count, callbacks, mutate);
        let mut result = Vec::new();
        for step in steps {
            let mut input = step
                .as_ref()
                .map(|bytes| CString::new(bytes.clone()).unwrap().into_bytes_with_nul());
            let ptr = input
                .as_mut()
                .map_or(std::ptr::null_mut(), |bytes| bytes.as_mut_ptr().cast());
            let mut state = [0u64; 128];
            let n = oracle_set_locale_step(rust, ptr, state.as_mut_ptr());
            result.push(state[..n].to_vec());
        }
        oracle_set_locale_finish();
        result
    }
}
fn compare(order: &[u8; 6], count: u32, callbacks: u32, mutate: bool, steps: &[Option<Vec<u8>>]) {
    let expected = run(false, order, count, callbacks, mutate, steps);
    let actual = run(true, order, count, callbacks, mutate, steps);
    assert_eq!(
        actual, expected,
        "order={order:?}, count={count}, callbacks={callbacks}, mutate={mutate}, steps={steps:?}"
    );
    // Pin the snapshot semantics in addition to comparing both sides.
    for (state, step) in actual.iter().zip(steps) {
        assert_eq!(state[0], u64::from(step.is_some()));
        assert_eq!(state[1], u64::from(step.is_some()));
        let event_len = state[2] as usize;
        for event in state[3..3 + event_len].chunks_exact(4) {
            assert!(event[0] < 6, "disabled-list callback must not run");
            assert_eq!(&event[1..], &[1, 1, u64::from(step.is_some())]);
        }
        if let Some(bytes) = step {
            assert_eq!(
                &state[3 + event_len..3 + event_len + bytes.len()],
                bytes
                    .iter()
                    .map(|byte| u64::from(*byte))
                    .collect::<Vec<_>>()
            );
        }
    }
}
#[test]
fn original_c_ownership_order_null_and_mutated_links() {
    // Allocation counters are process-wide: isolate this case in the existing
    // category executable instead of racing other tests or adding a binary.
    const CHILD: &str = "OBS_SET_LOCALE_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!(
                    "{}::original_c_ownership_order_null_and_mutated_links",
                    module_path!().split_once("::").unwrap().1
                ),
            ])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    // SAFETY: this isolated child has no other bmem users.
    let baseline = unsafe { obs_util::ffi::bmem::bnum_allocs() };
    let steps = [
        Some(b"en-US".to_vec()),
        Some(b"a-much-longer-replacement-locale".to_vec()),
        Some(vec![]),
        None,
        None,
        Some(b"fr-FR".to_vec()),
        Some(b"fr-FR".to_vec()),
        None,
    ];
    for count in [0, 1, 6] {
        for callbacks in [0, 1, 0x7f, 0x13] {
            for mutate in [false, true] {
                compare(&[5, 2, 3, 0, 1, 4], count, callbacks, mutate, &steps);
            }
        }
    }
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &(
                any::<[u16; 6]>(),
                0u32..7,
                0u32..128,
                any::<bool>(),
                prop::collection::vec(
                    prop::option::of(prop::collection::vec(1u8..=255, 0..65)),
                    1..17,
                ),
            ),
            |(keys, count, callbacks, mutate, steps)| {
                let mut order = [0, 1, 2, 3, 4, 5];
                order.sort_by_key(|index| keys[usize::from(*index)]);
                compare(&order, count, callbacks, mutate, &steps);
                Ok(())
            },
        )
        .unwrap();
    // SAFETY: all fixture-owned strings were freed and the child is isolated.
    assert_eq!(unsafe { obs_util::ffi::bmem::bnum_allocs() }, baseline);
}
#[test]
fn callback_layout_matches_real_c_header() {
    let mut layout = [0usize; 2];
    // SAFETY: the oracle writes two size_t values measured from the C header.
    unsafe { oracle_set_locale_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<obs_core::ffi::set_locale::Callback>(),
            align_of::<obs_core::ffi::set_locale::Callback>()
        ]
    );
}

//! Tier 3: unchanged original C versus the production Rust bridge/core with
//! the same disconnect recorder. No absent-core cases (C UB); an initialized
//! core without audio is defined. No intentional behavior differences.
use core::ffi::c_void;
use obs_core::ffi::raw_audio_remove::{Callback, Calls, remove_with};
use obs_media_io::ffi::audio_io::audio_data;
use proptest::prelude::*;

type Entry = unsafe extern "C" fn(usize, Callback, *mut c_void);
unsafe extern "C" {
    fn oracle_obs_remove_raw_audio_callback(mix: usize, callback: Callback, param: *mut c_void);
    fn obs_get_audio() -> *mut c_void;
    fn oracle_raw_audio_remove_disconnect(
        audio: *mut c_void,
        mix: usize,
        callback: Callback,
        param: *mut c_void,
    );
    fn oracle_raw_audio_remove_run(
        entry: Entry,
        masks: *const usize,
        has_audio: bool,
        mix: usize,
        callback: usize,
        param: usize,
        repeats: usize,
        report: *mut usize,
    );
    fn oracle_raw_audio_remove_layout(out: *mut usize);
}

unsafe extern "C" fn rust_entry(mix: usize, callback: Callback, param: *mut c_void) {
    // SAFETY: the C fixture supplies the live core and opaque tuple; the shared
    // recorder accepts all tuples, including NULL, and never calls callbacks.
    unsafe {
        remove_with(
            mix,
            callback,
            param,
            &mut Calls {
                main_audio: obs_get_audio,
                disconnect: oracle_raw_audio_remove_disconnect,
            },
        )
    };
}

fn compare(
    masks: [usize; 6],
    has_audio: bool,
    mix: usize,
    callback: usize,
    param: usize,
    repeats: usize,
) {
    let mut c = [0usize; 16];
    let mut rust = [0usize; 16];
    for (entry, report) in [
        (oracle_obs_remove_raw_audio_callback as Entry, &mut c),
        (rust_entry as Entry, &mut rust),
    ] {
        // SAFETY: six masks, a 16-slot report, valid identity indices and 1..=3
        // repeats. The fixture resets its owned real-header core after each run.
        unsafe {
            oracle_raw_audio_remove_run(
                entry,
                masks.as_ptr(),
                has_audio,
                mix,
                callback,
                param,
                repeats,
                report.as_mut_ptr(),
            )
        };
    }
    assert_eq!(rust, c, "mix={mix} callback={callback} param={param}");
    assert_eq!(c[0], repeats);
    assert_eq!(&c[1..5], &[usize::from(has_audio), mix, callback, param]);
    assert_eq!(c[11], 1, "wrapper mutated the core");
}

#[test]
fn original_c_arguments_calls_and_registration_state() {
    const CHILD: &str = "OBS_RAW_AUDIO_REMOVE_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(format!(
                "{}::original_c_arguments_calls_and_registration_state",
                module_path!().split_once("::").unwrap().1
            ))
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let mut layout = [0usize; 12];
    // SAFETY: the C header probe writes exactly twelve size/align/offset values.
    unsafe { oracle_raw_audio_remove_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<usize>(),
            align_of::<usize>(),
            size_of::<Callback>(),
            align_of::<Callback>(),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>(),
            size_of::<audio_data>(),
            align_of::<audio_data>(),
            std::mem::offset_of!(audio_data, data),
            std::mem::offset_of!(audio_data, frames),
            std::mem::offset_of!(audio_data, timestamp),
            6
        ]
    );
    let mut mixes = vec![0, 1, 2, 3, 4, 5, 6, 7, usize::MAX];
    if let Some(wide) = usize::try_from(u32::MAX).unwrap().checked_add(1) {
        mixes.extend([wide, wide + 1]);
    }
    for has_audio in [false, true] {
        for &mix in &mixes {
            for callback in 0..4 {
                for param in 0..4 {
                    compare(
                        [0xffff, 0x8421, 0, 0xffff, 0x1248, 0xffff],
                        has_audio,
                        mix,
                        callback,
                        param,
                        3,
                    );
                }
            }
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Default::default()
    });
    runner
        .run(
            &(
                any::<[u16; 6]>(),
                any::<bool>(),
                prop_oneof![0usize..8, any::<usize>()],
                0usize..4,
                0usize..4,
                1usize..4,
            ),
            |(masks, has_audio, mix, callback, param, repeats)| {
                compare(
                    masks.map(usize::from),
                    has_audio,
                    mix,
                    callback,
                    param,
                    repeats,
                );
                Ok(())
            },
        )
        .unwrap();
}

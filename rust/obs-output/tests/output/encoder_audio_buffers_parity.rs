//! Differential: original static C bodies versus production Rust shims,
//! both borrowing real C encoders. No behavior differences are intended.
use obs_c_oracle as _;
use obs_output::ffi::encoder_audio_buffers::View;
use proptest::prelude::*;
use std::{
    ffi::c_void,
    mem::{align_of, offset_of, size_of},
};
unsafe extern "C" {
    fn oracle_encoder_buffers_new(
        rate: u32,
        format: i32,
        speakers: i32,
        frames: usize,
        mutate: bool,
    ) -> *mut c_void;
    fn oracle_encoder_buffers_rust_run(fixture: *mut c_void, operation: i32);
    fn oracle_encoder_buffers_run(fixture: *mut c_void, operation: i32);
    fn oracle_encoder_buffers_snapshot(fixture: *mut c_void, out: *mut usize);
    fn oracle_encoder_buffers_delete(fixture: *mut c_void);
    fn oracle_encoder_buffers_layout(out: *mut usize);
}
struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: uniquely owned C fixture; delete frees every slot and fixture.
        unsafe {
            oracle_encoder_buffers_delete(self.0);
        }
    }
}
#[test]
fn real_header_layout() {
    let mut c = [0; 12];
    // SAFETY: C writes exactly twelve size_t values.
    unsafe {
        oracle_encoder_buffers_layout(c.as_mut_ptr());
    }
    assert_eq!(
        c,
        [
            size_of::<View>(),
            align_of::<View>(),
            offset_of!(View, input),
            offset_of!(View, output),
            offset_of!(View, rate),
            offset_of!(View, planes),
            offset_of!(View, blocksize),
            offset_of!(View, frames),
            offset_of!(View, bytes),
            offset_of!(View, frame_size),
            offset_of!(View, context),
            8
        ]
    );
}
fn compare(rate: u32, format: i32, speakers: i32, frames: usize, mutate: bool, operations: &[i32]) {
    // SAFETY: fixtures allocate all eight planes and valid callback/context;
    // bounded frames keep all oracle allocations small. View fields borrow r.
    unsafe {
        let c = Fixture(oracle_encoder_buffers_new(
            rate, format, speakers, frames, mutate,
        ));
        let r = Fixture(oracle_encoder_buffers_new(
            rate, format, speakers, frames, mutate,
        ));
        for &operation in operations {
            oracle_encoder_buffers_run(c.0, operation);
            oracle_encoder_buffers_rust_run(r.0, operation);
            let mut expected = [0; 58];
            let mut actual = [0; 58];
            oracle_encoder_buffers_snapshot(c.0, expected.as_mut_ptr());
            oracle_encoder_buffers_snapshot(r.0, actual.as_mut_ptr());
            assert_eq!(actual, expected);
        }
    }
}
#[test]
fn characterized_geometry_and_repeated_lifecycle() {
    for (format, speakers, rate, frames) in [
        (8, 2, 48000, 1024),
        (2, 2, 44100, 960),
        (6, 6, 32000, 17),
        (1, 1, 22050, 1),
    ] {
        compare(
            rate,
            format,
            speakers,
            frames,
            false,
            &[2, 1, 1, 0, 0, 2, 0],
        );
    }
    compare(48000, 8, 6, 17, true, &[2, 1, 0, 2, 0]);
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(64), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_lifecycle(rate in any::<u32>(), format in 1i32..9, speakers in prop::sample::select(vec![1,2,3,4,5,6,8]), frames in 1usize..2049, mutate in any::<bool>(), operations in prop::collection::vec(0i32..3, 1..20)) {
        compare(rate,format,speakers,frames,mutate,&operations);
    }
}

#[test]
fn allocation_balance() {
    if !super::isolated(&format!("{}::allocation_balance", module_path!())) {
        return;
    }
    // SAFETY: this child is the only test touching the allocation counter.
    let before = unsafe { obs_util::ffi::bmem::bnum_allocs() };
    compare(48000, 8, 2, 1024, false, &[2, 1, 1, 0, 0, 2, 0]);
    // SAFETY: all C and Rust fixture buffers have been freed before this read.
    assert_eq!(unsafe { obs_util::ffi::bmem::bnum_allocs() }, before);
}

#[test]
fn zero_byte_allocations_trap_like_c() {
    const PROBE: &str = "OBS_ENCODER_AUDIO_ZERO_PROBE";
    if let Ok(probe) = std::env::var(PROBE) {
        #[cfg(unix)]
        {
            let limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            // SAFETY: valid limit pointer; only this fatal-probe child is affected.
            assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) }, 0);
        }
        let (implementation, case) = probe.split_once(':').unwrap();
        let (format, speakers, frames) = match case {
            "frames" => (8, 2, 0),
            "format" => (0, 2, 1),
            _ => (2, 0, 1),
        };
        // SAFETY: fixture is valid. Original bmalloc(0) deliberately traps;
        // this is an isolated child of the existing category executable.
        unsafe {
            let fixture = Fixture(oracle_encoder_buffers_new(
                48000, format, speakers, frames, false,
            ));
            if implementation == "c" {
                oracle_encoder_buffers_run(fixture.0, 2);
            } else {
                oracle_encoder_buffers_rust_run(fixture.0, 2);
            }
        }
        return;
    }
    let test = format!("{}::zero_byte_allocations_trap_like_c", module_path!());
    let test = test
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap();
    for case in ["frames", "format", "speakers"] {
        let child = |implementation| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([test, "--exact", "--test-threads=1"])
                .env(PROBE, format!("{implementation}:{case}"))
                .output()
                .unwrap()
        };
        let c = child("c");
        let rust = child("rust");
        assert!(!c.status.success());
        assert_eq!(rust.status, c.status, "zero-byte {case}");
    }
}

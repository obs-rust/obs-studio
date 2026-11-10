//! Tier 3: original extracted C and Rust over the real output/track/deque layouts.
//! Inputs contain complete triplets and live tracks; C pointer/length UB is excluded.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_caption_queue::{Captions, obs_output_caption};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_output_caption_queue_layout(layout: *mut usize);
    fn oracle_output_caption_queue_new(mask: u32) -> *mut c_void;
    fn oracle_output_caption_queue_run(
        output: *mut c_void,
        data: *const u8,
        packets: u32,
        rust: bool,
    );
    fn oracle_output_caption_queue_read(
        output: *mut c_void,
        index: usize,
        data: *mut u8,
        capacity: usize,
    ) -> usize;
    fn oracle_output_caption_queue_free(output: *mut c_void);
}
struct Output(*mut c_void);
impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: exclusively owned fixture, releasing every deque and mutex.
        unsafe { oracle_output_caption_queue_free(self.0) };
    }
}

#[test]
fn public_caption_layout() {
    let mut layout = [0; 5];
    // SAFETY: oracle writes five size_t values from the real public header.
    unsafe { oracle_output_caption_queue_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Captions>(),
            align_of::<Captions>(),
            offset_of!(Captions, data),
            offset_of!(Captions, packets),
            offset_of!(Captions, timestamp)
        ]
    );
}

fn check(mask: u32, batches: &[Vec<[u8; 3]>]) {
    // SAFETY: constructor initializes each selected track and its mutex.
    let rust = Output(unsafe { oracle_output_caption_queue_new(mask) });
    // SAFETY: independent C fixture with the same track configuration.
    let c = Output(unsafe { oracle_output_caption_queue_new(mask) });
    let mut expected = Vec::new();
    for batch in batches {
        let mut input = batch.clone();
        let data = if input.is_empty() {
            core::ptr::null()
        } else {
            input.as_ptr().cast()
        };
        // SAFETY: both fixtures live; count describes complete triplets in input.
        unsafe {
            oracle_output_caption_queue_run(rust.0, data, input.len() as u32, true);
            oracle_output_caption_queue_run(c.0, data, input.len() as u32, false);
        }
        expected.extend(batch.iter().flatten().copied());
        input.fill([255; 3]);
        for index in 0..6 {
            let mut actual = vec![0; expected.len()];
            let mut original = vec![0; expected.len()];
            // SAFETY: buffers hold the full sum of all appended batch sizes.
            let (rlen, clen) = unsafe {
                (
                    oracle_output_caption_queue_read(
                        rust.0,
                        index,
                        actual.as_mut_ptr(),
                        actual.len(),
                    ),
                    oracle_output_caption_queue_read(
                        c.0,
                        index,
                        original.as_mut_ptr(),
                        original.len(),
                    ),
                )
            };
            assert!(rlen <= actual.len());
            assert!(clen <= original.len());
            assert_eq!(rlen, clen);
            actual.truncate(rlen);
            original.truncate(clen);
            assert_eq!(actual, original);
            assert_eq!(
                actual,
                if mask & (1 << index) != 0 {
                    expected.as_slice()
                } else {
                    &[]
                }
            );
        }
    }
}

#[test]
fn characterized_sparse_tracks_copy_and_append() {
    for mask in [0, 1, 32, 21, 63] {
        check(
            mask,
            &[
                vec![[9, 8, 7]],
                vec![],
                vec![[4, 193, 194], [4, 67, 196]],
                vec![[5, 6, 7]],
                vec![],
            ],
        );
    }
}

#[test]
fn no_tracks_does_not_read_captions() {
    // SAFETY: initialized output with no configured tracks.
    let output = Output(unsafe { oracle_output_caption_queue_new(0) });
    // SAFETY: no tracks means the C function and Rust core never inspect captions.
    unsafe { obs_output_caption(output.0, core::ptr::null()) };
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_batches_match_c(mask in 0u32..64, batches in prop::collection::vec(prop::collection::vec(any::<[u8; 3]>(), 0..33), 0..9)) {
        check(mask, &batches);
    }
}

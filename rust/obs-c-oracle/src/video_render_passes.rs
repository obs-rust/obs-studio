//! Serialized recording harness shared by the original C and the Rust ABI.
use std::ffi::c_char;
use std::sync::Mutex;

static RECORDING: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn oracle_video_render_passes(
        rust: bool,
        convert: bool,
        index: usize,
        count: usize,
        width: u32,
        height: u32,
        plane: usize,
        output: *mut c_char,
    ) -> usize;
}

/// Execute either implementation with bounded recording storage and real adapters.
pub fn trace(
    rust: bool,
    convert: bool,
    index: usize,
    count: usize,
    size: [u32; 2],
    plane: usize,
) -> String {
    assert!(index < 2 && count < 32 && plane < 3);
    let _guard = RECORDING.lock().unwrap();
    let mut output = [0u8; 512];
    // SAFETY: the lock protects the C recording state, indices are checked and
    // at most 32 passes produce fewer than 512 recorded calls.
    let len = unsafe {
        oracle_video_render_passes(
            rust,
            convert,
            index,
            count,
            size[0],
            size[1],
            plane,
            output.as_mut_ptr().cast(),
        )
    };
    String::from_utf8(output[..len].to_vec()).unwrap()
}

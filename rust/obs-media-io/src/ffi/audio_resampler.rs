//! Original resampler C ABI and FFmpeg kernel boundary. No FFmpeg structure
//! layout is assumed by Rust; version-dependent layout allocation stays in C.
use super::audio_io::resample_info;
use crate::audio_resampler::{Event, Host, Info, Resampler};
use core::{ffi::c_void, ptr};

unsafe extern "C" {
    fn obs_audio_resampler_alloc(
        dst: *const resample_info,
        src: *const resample_info,
    ) -> *mut c_void;
    fn obs_audio_resampler_log(kind: i32, code: i32, input: u64, output: u64, rate: f64);
    fn swr_set_matrix(context: *mut c_void, matrix: *const f64, stride: i32) -> i32;
    fn swr_init(context: *mut c_void) -> i32;
    fn swr_set_compensation(context: *mut c_void, delta: i32, distance: i32) -> i32;
    fn swr_get_delay(context: *mut c_void, base: i64) -> i64;
    fn av_rescale_rnd(value: i64, numerator: i64, denominator: i64, round: i32) -> i64;
    fn av_samples_alloc(
        planes: *mut *mut u8,
        linesize: *mut i32,
        channels: i32,
        samples: i32,
        format: i32,
        align: i32,
    ) -> i32;
    fn av_freep(ptr: *mut c_void);
    fn swr_free(context: *mut *mut c_void);
    fn swr_convert(
        context: *mut c_void,
        output: *mut *mut u8,
        capacity: i32,
        input: *const *const u8,
        frames: i32,
    ) -> i32;
}

fn description(info: Info) -> resample_info {
    resample_info {
        samples_per_sec: info.rate,
        format: info.format,
        speakers: info.speakers,
    }
}
fn info(raw: resample_info) -> Info {
    Info {
        rate: raw.samples_per_sec,
        format: raw.format,
        speakers: raw.speakers,
    }
}

struct Adapter {
    context: *mut c_void,
    output: [*mut u8; 8],
}
// The private input token can only be constructed by the unsafe C entry point.
struct Input(*const *const u8);
impl Host for Adapter {
    type Input = Input;
    fn alloc(&mut self, dst: Info, src: Info) -> bool {
        // SAFETY: stack descriptions live through the call; C retains neither.
        self.context = unsafe { obs_audio_resampler_alloc(&description(dst), &description(src)) };
        !self.context.is_null()
    }
    fn matrix(&mut self, row: &[f64; 8]) -> i32 {
        // SAFETY: initialized context and a complete eight-channel row.
        unsafe { swr_set_matrix(self.context, row.as_ptr(), 1) }
    }
    fn init(&mut self) -> i32 {
        // SAFETY: alloc succeeded.
        unsafe { swr_init(self.context) }
    }
    fn compensate(&mut self, delta: i32, distance: i32) -> i32 {
        // SAFETY: owned initialized context.
        unsafe { swr_set_compensation(self.context, delta, distance) }
    }
    fn delay(&mut self, base: i64) -> i64 {
        // SAFETY: owned initialized context; bases are the input rate or 1e9.
        unsafe { swr_get_delay(self.context, base) }
    }
    fn rescale(&mut self, value: i64, n: i64, d: i64) -> i64 {
        // SAFETY: FFmpeg integer helper; swr_init rejected invalid rates.
        unsafe { av_rescale_rnd(value, n, d, 3) }
    }
    fn resize(&mut self, channels: u32, frames: i32, format: i32) {
        // SAFETY: FFmpeg allocated plane zero; av_freep accepts NULL. The
        // allocation fills at most eight pointers, with the original alignment.
        unsafe {
            av_freep(ptr::from_mut(&mut self.output[0]).cast());
            av_samples_alloc(
                self.output.as_mut_ptr(),
                ptr::null_mut(),
                channels.cast_signed(),
                frames,
                format,
                0,
            );
        }
    }
    fn convert(&mut self, capacity: i32, input: Input, frames: i32) -> i32 {
        // SAFETY: Input is private and comes only from the C caller's valid
        // input planes. Output belongs to this adapter and has capacity frames.
        unsafe {
            swr_convert(
                self.context,
                self.output.as_mut_ptr(),
                capacity,
                input.0,
                frames,
            )
        }
    }
    fn log(&mut self, event: Event) {
        let (kind, code, input, output, rate) = match event {
            Event::Allocation => (0, 0, 0, 0, 0.0),
            Event::Matrix => (1, 0, 0, 0, 0.0),
            Event::Init(e) => (2, e, 0, 0, 0.0),
            Event::Engage => (3, 0, 0, 0, 0.0),
            Event::Compensation(e) => (4, e, 0, 0, 0.0),
            Event::Convert(e) => (5, e, 0, 0, 0.0),
            Event::Drift(i, o, r) => (6, 0, i, o, r),
        };
        // SAFETY: scalar arguments; formatting and log levels stay in C.
        unsafe { obs_audio_resampler_log(kind, code, input, output, rate) };
    }
}
impl Drop for Adapter {
    fn drop(&mut self) {
        // SAFETY: both resources are exclusively owned here. FFmpeg accepts
        // NULL after failed/partial initialization.
        unsafe {
            swr_free(&mut self.context);
            av_freep(ptr::from_mut(&mut self.output[0]).cast());
        }
    }
}

/// Opaque resampler handle. Only the four public functions access its storage.
pub struct AudioResampler(Resampler<Adapter>);

/// # Safety
/// Both descriptions must point to initialized resample_info objects.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_resampler_create(
    dst: *const resample_info,
    src: *const resample_info,
) -> *mut AudioResampler {
    // SAFETY: caller supplies live descriptions. NULL is rejected defensively.
    let (Some(dst), Some(src)) = (unsafe { dst.as_ref() }, unsafe { src.as_ref() }) else {
        return ptr::null_mut();
    };
    Resampler::create(
        Adapter {
            context: ptr::null_mut(),
            output: [ptr::null_mut(); 8],
        },
        info(*dst),
        info(*src),
    )
    .map_or(ptr::null_mut(), |rs| {
        Box::into_raw(Box::new(AudioResampler(rs)))
    })
}
/// # Safety
/// Handle is NULL or created here, exclusively owned, and never used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_resampler_destroy(rs: *mut AudioResampler) {
    if !rs.is_null() {
        // SAFETY: balances Box::into_raw in create.
        drop(unsafe { Box::from_raw(rs) });
    }
}
/// # Safety
/// Handle is NULL or live and exclusively borrowed for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_resampler_set_compensation_error(
    rs: *mut AudioResampler,
    error_ns: i64,
) {
    // SAFETY: caller supplies the handle; NULL is explicitly allowed.
    if let Some(rs) = unsafe { rs.as_mut() } {
        rs.0.set_error(error_ns)
    }
}
/// # Safety
/// For a non-NULL live, exclusively borrowed handle, input contains the
/// configured planes with in_frames samples. Output has room for the configured
/// output planes; out_frames and ts_offset are writable. Output samples remain
/// valid until the next resample or destroy call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_resampler_resample(
    rs: *mut AudioResampler,
    output: *mut *mut u8,
    out_frames: *mut u32,
    ts_offset: *mut u64,
    input: *const *const u8,
    in_frames: u32,
) -> bool {
    // SAFETY: caller's handle, with the original NULL early return contract.
    let Some(rs) = (unsafe { rs.as_mut() }) else {
        return false;
    };
    // SAFETY: writable timestamp; input validity is required by this entry point.
    let result =
        rs.0.resample(Input(input), in_frames, unsafe { &mut *ts_offset });
    let Some(frames) = result else { return false };
    // SAFETY: output has the configured plane count. Extra slots stay untouched.
    unsafe {
        ptr::copy_nonoverlapping(
            rs.0.host.output.as_ptr(),
            output,
            usize::try_from(rs.0.planes).unwrap_or(0),
        );
        *out_frames = frames;
    }
    true
}

/// Checked byte-plane frontend for safe-core characterization tests. It drives
/// the very same host and core as the ABI, without exposing a raw input token.
#[cfg(test)]
pub(crate) struct CheckedResampler {
    core: Resampler<Adapter>,
    src: Info,
    dst: Info,
}
#[cfg(test)]
impl CheckedResampler {
    pub(crate) fn create(dst: Info, src: Info) -> Option<Self> {
        if !valid_info(src) || !valid_info(dst) {
            return None;
        }
        Some(Self {
            core: Resampler::create(
                Adapter {
                    context: ptr::null_mut(),
                    output: [ptr::null_mut(); 8],
                },
                dst,
                src,
            )?,
            src,
            dst,
        })
    }
    pub(crate) fn set_error(&mut self, error: i64) {
        self.core.set_error(error);
    }
    pub(crate) fn resample(
        &mut self,
        input: &[Vec<u8>],
        frames: u32,
    ) -> Option<(u32, u64, Vec<Vec<u8>>)> {
        let (planes, stride) = shape(self.src);
        let bytes = usize::try_from(frames).ok()?.checked_mul(stride)?;
        if input.len() != planes || input.iter().any(|p| p.len() < bytes) {
            return None;
        }
        let mut raw = [ptr::null(); 8];
        for (slot, data) in raw.iter_mut().zip(input) {
            *slot = data.as_ptr();
        }
        let mut offset = u64::MAX;
        let count = self
            .core
            .resample(Input(raw.as_ptr()), frames, &mut offset)?;
        let (planes, stride) = shape(self.dst);
        let bytes = usize::try_from(count).ok()?.checked_mul(stride)?;
        let mut result = Vec::new();
        for &plane in self.core.host.output.iter().take(planes) {
            if bytes == 0 {
                result.push(Vec::new());
                continue;
            }
            // SAFETY: FFmpeg returned count frames in its own output allocation,
            // and the format/plane dimensions were checked before construction.
            result.push(unsafe { core::slice::from_raw_parts(plane, bytes) }.to_vec());
        }
        Some((count, offset, result))
    }
}
#[cfg(test)]
fn valid_info(info: Info) -> bool {
    (0..=8).contains(&info.format)
        && crate::audio_resampler_drift::output_channels(info.speakers) > 0
}
#[cfg(test)]
fn shape(info: Info) -> (usize, usize) {
    let channels =
        usize::try_from(crate::audio_resampler_drift::output_channels(info.speakers)).unwrap_or(0);
    let planar = crate::audio_resampler_drift::planar_output(info.format);
    let width = match info.format {
        1 | 5 => 1,
        0 | 2 | 6 => 2,
        _ => 4,
    };
    (
        if planar { channels } else { 1 },
        width * if planar { 1 } else { channels },
    )
}

#[cfg(test)]
pub(crate) unsafe fn snapshot(
    rs: *mut AudioResampler,
) -> crate::audio_resampler_drift::Compensation {
    // SAFETY: test caller holds the live exclusive handle.
    unsafe { (*rs).0.compensation }
}

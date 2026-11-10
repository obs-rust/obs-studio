//! C ABI conversion over borrowed encoder fields; caller retains ownership.
use crate::encoder_configuration::{self as core, Configuration, Effects, Request};
use ::core::ffi::{CStr, c_char, c_int, c_void};
use ::core::sync::atomic::{AtomicBool, Ordering};

#[repr(C)]
pub struct Fields {
    pub kind: *const c_int,
    pub active: *mut bool,
    pub initialized: *const bool,
    pub width: *mut u32,
    pub height: *mut u32,
    pub gpu: *mut c_int,
    pub divisor: *mut u32,
    pub media: *mut c_void,
    pub fps_override: *mut *mut c_void,
    pub name: *const c_char,
}
unsafe extern "C" {
    fn obs_encoder_configuration_fields(encoder: *mut c_void) -> Fields;
    fn obs_encoder_configuration_output_size(media: *mut c_void, w: *mut u32, h: *mut u32) -> bool;
    fn obs_encoder_configuration_free_override(video: *mut c_void);
    fn obs_encoder_configuration_create_override(media: *mut c_void, divisor: u32) -> *mut c_void;
    fn blog(level: c_int, format: *const c_char, ...);
}
struct Encoder(Option<Fields>);
impl Effects for Encoder {
    fn log(&mut self, level: i32, format: &'static CStr) {
        let name = self.0.as_ref().map_or(::core::ptr::null(), |f| f.name);
        // SAFETY: formats are static C strings; name is borrowed for this synchronous call.
        unsafe {
            blog(level, format.as_ptr(), name);
        }
    }
    fn output_size(&mut self) -> Option<(u32, u32)> {
        let f = self.0.as_ref()?;
        let (mut w, mut h) = (0, 0);
        // SAFETY: adapter accepts NULL media and writes the two local dimensions.
        unsafe { obs_encoder_configuration_output_size(f.media, &mut w, &mut h) }.then_some((w, h))
    }
    fn scaled_size(&mut self, width: u32, height: u32) {
        let f = self.0.as_ref().expect("validated encoder");
        // SAFETY: caller exclusively holds these writable encoder fields.
        unsafe {
            *f.width = width;
            *f.height = height;
        }
    }
    fn gpu_scale(&mut self, scale: i32) {
        let f = self.0.as_ref().expect("validated encoder");
        // SAFETY: enum has C int layout and scale came from the C ABI.
        unsafe {
            *f.gpu = scale;
        }
    }
    fn divisor(&mut self, divisor: u32) {
        let f = self.0.as_ref().expect("validated encoder");
        // SAFETY: caller exclusively holds the writable divisor field.
        unsafe {
            *f.divisor = divisor;
        }
    }
    fn free_override(&mut self) {
        let f = self.0.as_ref().expect("validated encoder");
        // SAFETY: override belongs to this encoder; clear it immediately after freeing.
        unsafe {
            obs_encoder_configuration_free_override(*f.fps_override);
            *f.fps_override = ::core::ptr::null_mut();
        }
    }
    fn create_override(&mut self, divisor: u32) {
        let f = self.0.as_ref().expect("validated encoder");
        // SAFETY: media remains live and the resulting override becomes encoder-owned.
        unsafe {
            *f.fps_override = obs_encoder_configuration_create_override(f.media, divisor);
        }
    }
}
unsafe fn configure(encoder: *mut c_void, request: Request) -> bool {
    let mut effects = Encoder(None);
    let state = if encoder.is_null() {
        None
    } else {
        // SAFETY: NULL was rejected before any C member address is taken. Caller
        // retains the encoder and synchronizes non-atomic configuration fields.
        unsafe {
            let f = obs_encoder_configuration_fields(encoder);
            let state = Configuration {
                kind: *f.kind,
                active: AtomicBool::from_ptr(f.active).load(Ordering::SeqCst),
                initialized: *f.initialized,
                has_override: !(*f.fps_override).is_null(),
                has_media: !f.media.is_null(),
            };
            effects.0 = Some(f);
            Some(state)
        }
    };
    core::configure(state, request, &mut effects)
}

/// # Safety
/// A non-null encoder is live and exclusively held for configuration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_set_scaled_size(
    encoder: *mut c_void,
    width: u32,
    height: u32,
) {
    // SAFETY: forwarded caller contract; NULL is handled by configure.
    unsafe {
        configure(encoder, Request::ScaledSize(width, height));
    }
}
/// # Safety
/// A non-null encoder is live and exclusively held for configuration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_set_gpu_scale_type(encoder: *mut c_void, scale: c_int) {
    // SAFETY: forwarded caller contract; NULL is handled by configure.
    unsafe {
        configure(encoder, Request::GpuScale(scale));
    }
}
/// # Safety
/// A non-null encoder is live and exclusively held for configuration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_set_frame_rate_divisor(
    encoder: *mut c_void,
    divisor: u32,
) -> bool {
    // SAFETY: forwarded caller contract; NULL is handled by configure.
    unsafe { configure(encoder, Request::Divisor(divisor)) }
}

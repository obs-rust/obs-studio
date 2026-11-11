//! Public encoder configuration orchestration; private layout stays in C.
use core::ffi::CStr;

#[derive(Clone, Copy, Debug)]
pub struct Configuration {
    pub kind: i32,
    pub active: bool,
    pub initialized: bool,
    pub has_override: bool,
    pub has_media: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Request {
    ScaledSize(u32, u32),
    GpuScale(i32),
    Divisor(u32),
}

/// Effects are synchronous and operate on the caller's exclusively held encoder.
pub trait Effects {
    fn log(&mut self, level: i32, format: &'static CStr);
    fn output_size(&mut self) -> Option<(u32, u32)>;
    fn scaled_size(&mut self, width: u32, height: u32);
    fn gpu_scale(&mut self, scale: i32);
    fn divisor(&mut self, divisor: u32);
    fn free_override(&mut self);
    fn create_override(&mut self, divisor: u32);
}

pub fn configure(
    state: Option<Configuration>,
    request: Request,
    effects: &mut impl Effects,
) -> bool {
    let Some(state) = state else {
        effects.log(
            400,
            match request {
                Request::ScaledSize(..) => c"obs_encoder_set_scaled_size: Null 'encoder' parameter",
                Request::GpuScale(_) => c"obs_encoder_set_gpu_scale_type: Null 'encoder' parameter",
                Request::Divisor(_) => {
                    c"obs_encoder_set_frame_rate_divisor: Null 'encoder' parameter"
                }
            },
        );
        return false;
    };
    let warning = if state.kind != 1 {
        Some(match request {
            Request::ScaledSize(..) => {
                c"obs_encoder_set_scaled_size: encoder '%s' is not a video encoder"
            }
            Request::GpuScale(_) => {
                c"obs_encoder_set_gpu_scale_type: encoder '%s' is not a video encoder"
            }
            Request::Divisor(_) => {
                c"obs_encoder_set_frame_rate_divisor: encoder '%s' is not a video encoder"
            }
        })
    } else if state.active {
        Some(match request {
            Request::ScaledSize(..) => {
                c"encoder '%s': Cannot set the scaled resolution while the encoder is active"
            }
            Request::GpuScale(_) => {
                c"encoder '%s': Cannot enable GPU scaling while the encoder is active"
            }
            Request::Divisor(_) => {
                c"encoder '%s': Cannot set frame rate divisor while the encoder is active"
            }
        })
    } else if state.initialized {
        Some(match request {
            Request::ScaledSize(..) => c"encoder '%s': Cannot set the scaled resolution after the encoder has been initialized",
            Request::GpuScale(_) => c"encoder '%s': Cannot enable GPU scaling after the encoder has been initialized",
            Request::Divisor(_) => c"encoder '%s': Cannot set frame rate divisor after the encoder has been initialized",
        })
    } else {
        None
    };
    if let Some(warning) = warning {
        effects.log(200, warning);
        return false;
    }
    match request {
        Request::ScaledSize(w, h) => {
            if effects.output_size() == Some((w, h)) {
                effects.log(
                    200,
                    c"encoder '%s': Scaled resolution matches output resolution, scaling disabled",
                );
                effects.scaled_size(0, 0);
            } else {
                effects.scaled_size(w, h);
            }
        }
        Request::GpuScale(scale) => effects.gpu_scale(scale),
        Request::Divisor(0) => {
            effects.log(200, c"encoder '%s': Cannot set frame rate divisor to 0");
            return false;
        }
        Request::Divisor(divisor) => {
            effects.divisor(divisor);
            if state.has_override {
                effects.free_override();
            }
            if state.has_media {
                effects.create_override(divisor);
            }
        }
    }
    true
}

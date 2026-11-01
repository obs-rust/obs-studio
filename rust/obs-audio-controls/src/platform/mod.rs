//! The platform boundary (obs-rust/obs-studio#323): the only place in the
//! safe core that selects on the host. Here it picks the AVX2 copy of the
//! true-peak loop at run time (#3415).

cfg_select! {
    target_arch = "x86_64" => {
        /// [`crate::peak::true_peak_scalar`] compiled for AVX2.
        #[target_feature(enable = "avx2")]
        fn true_peak_avx2(prev: [f32; 4], samples: &[f32], frames: usize) -> f32 {
            crate::peak::true_peak_scalar(prev, samples, frames)
        }

        /// Runs the AVX2 copy when the CPU has it, else the scalar loop.
        pub(crate) fn true_peak(prev: [f32; 4], samples: &[f32], frames: usize) -> f32 {
            if std::arch::is_x86_feature_detected!("avx2") {
                // SAFETY: the CPU was just checked for AVX2.
                #[allow(unsafe_code)]
                unsafe {
                    true_peak_avx2(prev, samples, frames)
                }
            } else {
                crate::peak::true_peak_scalar(prev, samples, frames)
            }
        }
    }
    _ => {
        pub(crate) fn true_peak(prev: [f32; 4], samples: &[f32], frames: usize) -> f32 {
            crate::peak::true_peak_scalar(prev, samples, frames)
        }
    }
}

//! The platform boundary (obs-rust/obs-studio#323): the only place in the
//! safe cores that selects on the host. Each OS child holds that host's
//! native code; the rest of the crate uses the neutral items exported here.
//! The `ban_platform_cfg_outside_boundary` Dylint rejects host `cfg`s and
//! native OS paths (`std::os::*`, `libc::`, Win32) outside this module and
//! [`crate::ffi`].

#[allow(unsafe_code)] // the cell is touched only while this thread holds the lock
mod re_mutex;
pub(crate) use re_mutex::ReMutex;

cfg_select! {
    unix => {
        #[allow(unsafe_code)] // pthread mutex calls; see the module docs
        mod unix;
        pub(crate) use unix::AudioThreadClass;
        pub(crate) use unix::RecursiveMutex;

        /// Expands its items only on hosts that build the `audio-io.c` port.
        macro_rules! with_audio_thread_port {
            ($($item:item)*) => { $($item)* };
        }
    }
    windows => {
        #[allow(unsafe_code)] // Win32 critical section; see the module docs
        mod windows;
        pub(crate) use windows::AudioThreadClass;
        pub(crate) use windows::RecursiveMutex;

        /// Expands its items only on hosts that build the `audio-io.c` port.
        macro_rules! with_audio_thread_port {
            ($($item:item)*) => { $($item)* };
        }
    }
    _ => {
        /// This host keeps the C file, so the items are dropped.
        macro_rules! with_audio_thread_port {
            ($($item:item)*) => {};
        }
    }
}

pub(crate) use with_audio_thread_port;

cfg_select! {
    target_arch = "x86_64" => {
        /// [`crate::audio_clamp::clamp_samples_scalar`] compiled for AVX2.
        #[target_feature(enable = "avx2")]
        fn clamp_samples_avx2(buf: &mut [f32]) {
            let _ = buf;
            todo!("AVX2 clamp (#3414)")
        }

        /// Runs the AVX2 copy when the CPU has it, else the scalar loop.
        pub(crate) fn clamp_samples(buf: &mut [f32]) {
            if std::arch::is_x86_feature_detected!("avx2") {
                // SAFETY: the CPU was just checked for AVX2.
                #[allow(unsafe_code)]
                unsafe {
                    clamp_samples_avx2(buf);
                }
            } else {
                crate::audio_clamp::clamp_samples_scalar(buf);
            }
        }
    }
    _ => {
        pub(crate) fn clamp_samples(buf: &mut [f32]) {
            crate::audio_clamp::clamp_samples_scalar(buf);
        }
    }
}

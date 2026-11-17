//! #4754: the three output callback setters given a NULL output. The C
//! originals dereference it, so this checks only the Rust shims: each logs
//! `obs_output_valid`'s debug line once and returns without touching memory.
use core::ffi::c_void;
use obs_output::ffi::output_conversion_callbacks::{
    Callback as PacketCallback, obs_output_add_packet_callback, obs_output_remove_packet_callback,
};
use obs_output::ffi::output_reconnect_callback::{Callback, obs_output_set_reconnect_callback};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};

type SetReconnect = unsafe extern "C" fn(*mut c_void, Option<Callback>, *mut c_void);
type SetPacket = unsafe extern "C" fn(*mut c_void, PacketCallback, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_callback_null_logs(
        set_reconnect: SetReconnect,
        add: SetPacket,
        remove: SetPacket,
        get: GetLog,
        set: SetLog,
    ) -> bool;
}

#[test]
fn null_output_setters_log_and_return() {
    if !super::isolated(concat!(
        module_path!(),
        "::null_output_setters_log_and_return"
    )) {
        return;
    }
    // SAFETY: the isolated child owns the log handler, which the probe
    // restores; every setter is called with a NULL output, which it must
    // refuse before reading anything.
    unsafe {
        assert!(oracle_output_callback_null_logs(
            obs_output_set_reconnect_callback,
            obs_output_add_packet_callback,
            obs_output_remove_packet_callback,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

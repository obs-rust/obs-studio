//! obs-hotkey: the Windows key map (`libobs/obs-windows-keys.c.h`). When the
//! hotkey platform starts it fills `vk_codes` with the virtual key of every
//! key (`obs_hotkeys_platform_init`), and the hotkey settings then ask for
//! keys of virtual keys (`obs_key_from_virtual_key`) and the name route of
//! each key they show (`obs_key_to_str`: the route, the scan code and the
//! system name gate). Each side fills the table and handles 64 keys, folded
//! into a checksum both sides must match. The tables are the bench's own;
//! nothing is allocated with `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::windows_keys::{
    oracle_obs_windows_keys_from_virtual_key, oracle_obs_windows_keys_name,
    oracle_obs_windows_keys_scan_code, oracle_obs_windows_keys_system_name,
    oracle_obs_windows_keys_virtual_key,
};
use obs_hotkey::ffi::windows_keys::{
    obs_windows_keys_from_virtual_key, obs_windows_keys_name, obs_windows_keys_scan_code,
    obs_windows_keys_system_name, obs_windows_keys_virtual_key,
};
use obs_hotkey::windows_keys::OBS_KEY_LAST_VALUE;
use std::ffi::c_int;
use std::hint::black_box;

const KEYS: usize = 64;

/// Key `i` of the 64: spread over the whole enum.
fn key(i: usize) -> c_int {
    c_int::try_from((i * 97) % OBS_KEY_LAST_VALUE).unwrap()
}

fn mix(acc: u32, value: u32) -> u32 {
    acc.wrapping_mul(31).wrapping_add(value)
}

fn rust_keys(vk_codes: &mut [c_int]) -> u32 {
    for (key, vk) in vk_codes.iter_mut().enumerate() {
        *vk = obs_windows_keys_virtual_key(black_box(c_int::try_from(key).unwrap()));
    }
    let mut acc = 0_u32;
    for i in 0..KEYS {
        let key = black_box(key(i));
        let vk = vk_codes[usize::try_from(key).unwrap()];
        // SAFETY: OBS_KEY_LAST_VALUE ints, alive for the call.
        let found = unsafe { obs_windows_keys_from_virtual_key(vk_codes.as_ptr(), vk) };
        let mut number = 0;
        // SAFETY: writes at most one int to a live local.
        let route = unsafe { obs_windows_keys_name(key, &mut number) };
        let scan = obs_windows_keys_scan_code(vk, u32::try_from(i).unwrap());
        let gate = obs_windows_keys_system_name(key, scan);
        acc = mix(acc, found.unsigned_abs());
        acc = mix(acc, route.unsigned_abs() ^ number.unsigned_abs());
        acc = mix(acc, scan ^ u32::from(gate));
    }
    acc
}

fn c_keys(vk_codes: &mut [c_int]) -> u32 {
    for (key, vk) in vk_codes.iter_mut().enumerate() {
        // SAFETY: the oracle reads only its argument.
        *vk = unsafe { oracle_obs_windows_keys_virtual_key(black_box(c_int::try_from(key).unwrap())) };
    }
    let mut acc = 0_u32;
    for i in 0..KEYS {
        let key = black_box(key(i));
        let vk = vk_codes[usize::try_from(key).unwrap()];
        // SAFETY: as rust_keys; the oracle keeps no state.
        let (found, route, number, scan, gate) = unsafe {
            let found = oracle_obs_windows_keys_from_virtual_key(vk_codes.as_ptr(), vk);
            let mut number = 0;
            let route = oracle_obs_windows_keys_name(key, &mut number);
            let scan = oracle_obs_windows_keys_scan_code(vk, u32::try_from(i).unwrap());
            let gate = oracle_obs_windows_keys_system_name(key, scan);
            (found, route, number, scan, gate)
        };
        acc = mix(acc, found.unsigned_abs());
        acc = mix(acc, route.unsigned_abs() ^ number.unsigned_abs());
        acc = mix(acc, scan ^ u32::from(gate));
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    let mut ours = vec![0; OBS_KEY_LAST_VALUE];
    let mut theirs = vec![0; OBS_KEY_LAST_VALUE];
    pair(
        criterion,
        "windows_keys/init_and_names/64",
        || rust_keys(&mut ours),
        || c_keys(&mut theirs),
    );
}

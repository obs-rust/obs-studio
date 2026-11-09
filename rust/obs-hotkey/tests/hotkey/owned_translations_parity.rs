//! Real C fields and unchanged C setter bodies; no intentional differences.
use core::ffi::c_char;
use obs_hotkey::ffi::owned_translations::{
    obs_hotkeys_set_audio_hotkeys_translations as audio,
    obs_hotkeys_set_sceneitem_hotkeys_translations as scene,
};
use proptest::prelude::*;
use std::ffi::CStr;

unsafe extern "C" {
    fn obs_hotkey_owned_translation_field(slot: usize) -> *mut *mut c_char;
    fn oracle_obs_hotkeys_set_audio_hotkeys_translations(
        a: *const c_char,
        b: *const c_char,
        c: *const c_char,
        d: *const c_char,
        e: *const c_char,
        f: *const c_char,
    );
    fn oracle_obs_hotkeys_set_sceneitem_hotkeys_translations(a: *const c_char, b: *const c_char);
    fn oracle_hotkey_owned_translations_clear();
    fn oracle_hotkey_owned_translations_layout(out: *mut usize);
}

#[test]
fn real_header_pointer_and_index_layout() {
    let mut layout = [0; 6];
    // SAFETY: the C oracle writes exactly six size_t measurements.
    unsafe { oracle_hotkey_owned_translations_layout(layout.as_mut_ptr()) };
    use core::mem::{align_of, size_of};
    assert_eq!(
        layout,
        [
            size_of::<*mut c_char>(),
            align_of::<*mut c_char>(),
            size_of::<*mut *mut c_char>(),
            align_of::<*mut *mut c_char>(),
            size_of::<usize>(),
            align_of::<usize>()
        ]
    );
}

type Step = (bool, [Option<Vec<u8>>; 6]);
struct World;
impl Drop for World {
    fn drop(&mut self) {
        // SAFETY: this thread owns all eight fixture fields and their bmem storage.
        unsafe { oracle_hotkey_owned_translations_clear() };
    }
}

fn run(steps: &[Step], rust: bool) -> Vec<[Option<Vec<u8>>; 8]> {
    let _world = World;
    let mut snapshots = Vec::new();
    for (is_audio, values) in steps {
        let mut buffers = values.clone().map(|v| {
            v.map(|mut bytes| {
                bytes.push(0);
                bytes
            })
        });
        let pointers = buffers.each_ref().map(|v| {
            v.as_ref()
                .map_or(core::ptr::null(), |v| v.as_ptr().cast::<c_char>())
        });
        // SAFETY: all inputs are null or terminated, disjoint from the owned
        // fixture fields. The original bodies and production adapter use the
        // same real-header thread-local core. Every allocated value is freed.
        unsafe {
            let [a, b, c, d, e, f] = pointers;
            match (*is_audio, rust) {
                (true, true) => audio(a, b, c, d, e, f),
                (true, false) => {
                    oracle_obs_hotkeys_set_audio_hotkeys_translations(a, b, c, d, e, f)
                }
                (false, true) => scene(a, b),
                (false, false) => oracle_obs_hotkeys_set_sceneitem_hotkeys_translations(a, b),
            }
            // Caller buffers can change immediately after the call.
            for bytes in buffers.iter_mut().flatten() {
                bytes.fill(b'x');
            }
            snapshots.push(std::array::from_fn(|slot| {
                let ptr = *obs_hotkey_owned_translation_field(slot);
                assert!(!pointers.contains(&ptr.cast_const()) || ptr.is_null());
                (!ptr.is_null()).then(|| CStr::from_ptr(ptr).to_bytes().to_vec())
            }));
        }
    }
    snapshots
}

fn compare(steps: &[Step]) {
    assert_eq!(run(steps, true), run(steps, false));
}

#[test]
fn all_slots_null_empty_replacement_and_groups_match_c() {
    let filled = std::array::from_fn(|i| Some(vec![b'A' + i as u8, 0xff, b'%', b'1']));
    let empty = std::array::from_fn(|_| Some(vec![]));
    let nulls = std::array::from_fn(|_| None);
    let mut steps = vec![(true, filled), (false, empty.clone())];
    for slot in 0..6 {
        let mut mixed = empty.clone();
        mixed[slot] = None;
        steps.push((true, mixed));
    }
    steps.extend([(false, nulls.clone()), (true, nulls), (true, empty)]);
    compare(&steps);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn differential(steps in prop::collection::vec((any::<bool>(), prop::array::uniform6(prop::option::of(prop::collection::vec(any::<u8>(), 0..64)))), 1..24)) {
        compare(&steps);
    }
}

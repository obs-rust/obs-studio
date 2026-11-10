//! No intentional differences from the original C introspection dispatchers.
use obs_c_oracle as _;
use obs_graphics::ffi::effect_passes::gs_shader_param_info;
use obs_graphics::ffi::shader_param_introspection as r;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};
use std::mem::{align_of, offset_of, size_of};

unsafe extern "C" {
    fn oracle_gs_shader_get_num_params(shader: *const c_void) -> i32;
    fn oracle_gs_shader_get_param_by_idx(shader: *mut c_void, index: u32) -> *mut c_void;
    fn oracle_gs_shader_get_param_by_name(shader: *mut c_void, name: *const c_char) -> *mut c_void;
    fn oracle_gs_shader_get_param_info(param: *const c_void, info: *mut gs_shader_param_info);
    fn oracle_spi_reset(
        present: bool,
        count: i32,
        found: bool,
        kind: i32,
        name: *const c_char,
        writes: u32,
    );
    fn oracle_spi_stop();
    fn oracle_spi_token(index: u32) -> *mut c_void;
    fn oracle_spi_calls(index: u32) -> u32;
    fn oracle_spi_seen(index: u32) -> *const c_void;
    fn oracle_spi_index() -> u32;
    fn oracle_spi_logs() -> u32;
    fn oracle_spi_level() -> i32;
    fn oracle_spi_text() -> *const c_char;
    fn oracle_spi_order() -> *const c_char;
    fn oracle_spi_layout(index: u32) -> usize;
    fn oracle_spi_type(index: u32) -> i32;
    fn oracle_spi_adapter_layout() -> bool;
}
struct Fixture;
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: restores this thread's existing effect backend fixture selection.
        unsafe { oracle_spi_stop() };
    }
}

#[test]
fn real_header_layout_and_callbacks() {
    let _fixture = Fixture;
    // SAFETY: the oracle owns all thread-local fixture storage and layout values.
    unsafe {
        oracle_spi_reset(true, 0, false, 0, std::ptr::null(), 0);
        let expected = [
            size_of::<gs_shader_param_info>(),
            align_of::<gs_shader_param_info>(),
            offset_of!(gs_shader_param_info, type_),
            offset_of!(gs_shader_param_info, name),
            size_of::<i32>(),
            align_of::<i32>(),
        ];
        for (index, value) in expected.into_iter().enumerate() {
            assert_eq!(oracle_spi_layout(index as u32), value);
        }
        for kind in 0..=12 {
            assert_eq!(oracle_spi_type(kind), kind as i32);
        }
        assert!(oracle_spi_adapter_layout());
    }
}

#[derive(Clone, Debug)]
struct Case {
    op: u8,
    present: bool,
    primary: bool,
    secondary: bool,
    count: i32,
    index: u32,
    found: bool,
    kind: i32,
    named: bool,
    writes: u32,
    empty: bool,
}
#[derive(Debug, PartialEq, Eq)]
struct Observation {
    count: i32,
    handle: usize,
    kind: i32,
    name: usize,
    calls: [u32; 4],
    seen: [usize; 4],
    index: u32,
    logs: u32,
    level: i32,
    text: Vec<u8>,
    order: Vec<u8>,
}

fn compare(case: &Case) {
    let _fixture = Fixture;
    let original = c"original".as_ptr();
    let backend = if case.named {
        c"backend name".as_ptr()
    } else {
        std::ptr::null()
    };
    let mut output: gs_shader_param_info;
    let mut observations = Vec::new();
    for rust in [false, true] {
        // SAFETY: opaque handles refer to live C-owned tokens, names to static C
        // strings and output to writable storage. The same output address is used
        // on both sides so exact callback pointer forwarding can be compared.
        unsafe {
            oracle_spi_reset(
                case.present,
                case.count,
                case.found,
                case.kind,
                backend,
                case.writes,
            );
            output = gs_shader_param_info {
                type_: 3,
                name: original,
            };
            let shader = if case.primary {
                oracle_spi_token(0)
            } else {
                std::ptr::null_mut()
            };
            let param = if case.primary {
                oracle_spi_token(1)
            } else {
                std::ptr::null_mut()
            };
            let name = if !case.secondary {
                std::ptr::null()
            } else if case.empty {
                c"".as_ptr()
            } else {
                c"parameter[7].value".as_ptr()
            };
            let info = if case.secondary {
                &mut output
            } else {
                std::ptr::null_mut()
            };
            let mut count = 0;
            let mut handle = std::ptr::null_mut();
            match (case.op, rust) {
                (0, false) => count = oracle_gs_shader_get_num_params(shader),
                (0, true) => count = r::gs_shader_get_num_params(shader),
                (1, false) => handle = oracle_gs_shader_get_param_by_idx(shader, case.index),
                (1, true) => handle = r::gs_shader_get_param_by_idx(shader, case.index),
                (2, false) => handle = oracle_gs_shader_get_param_by_name(shader, name),
                (2, true) => handle = r::gs_shader_get_param_by_name(shader, name),
                (3, false) => oracle_gs_shader_get_param_info(param, info),
                (3, true) => r::gs_shader_get_param_info(param, info),
                _ => unreachable!(),
            }
            observations.push(Observation {
                count,
                handle: handle as usize,
                kind: output.type_,
                name: output.name as usize,
                calls: std::array::from_fn(|i| oracle_spi_calls(i as u32)),
                seen: std::array::from_fn(|i| oracle_spi_seen(i as u32) as usize),
                index: oracle_spi_index(),
                logs: oracle_spi_logs(),
                level: oracle_spi_level(),
                text: CStr::from_ptr(oracle_spi_text()).to_bytes().to_vec(),
                order: CStr::from_ptr(oracle_spi_order()).to_bytes().to_vec(),
            });
        }
    }
    assert_eq!(observations[0], observations[1], "{case:?}");
}

#[test]
fn characterized_guards_results_and_out_writes() {
    for op in 0..4 {
        for present in [false, true] {
            for primary in [false, true] {
                for secondary in [false, true] {
                    for found in [false, true] {
                        for empty in [false, true] {
                            compare(&Case {
                                op,
                                present,
                                primary,
                                secondary,
                                count: -1,
                                index: u32::MAX,
                                found,
                                kind: 0,
                                named: true,
                                writes: 3,
                                empty,
                            });
                        }
                    }
                }
            }
        }
    }
    for count in [0, 1, -1, i32::MIN, i32::MAX] {
        for index in [0, 1, u32::MAX] {
            for op in 0..4 {
                for kind in 0..=12 {
                    for writes in 0..4 {
                        for named in [false, true] {
                            compare(&Case {
                                op,
                                present: true,
                                primary: true,
                                secondary: true,
                                count,
                                index,
                                found: true,
                                kind,
                                named,
                                writes,
                                empty: false,
                            });
                        }
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn randomized_original_c_parity(
        op in 0u8..4, present in any::<bool>(), primary in any::<bool>(),
        secondary in any::<bool>(), count in any::<i32>(), index in any::<u32>(),
        found in any::<bool>(), kind in 0i32..13, named in any::<bool>(),
        writes in 0u32..4, empty in any::<bool>(),
    ) {
        compare(&Case { op, present, primary, secondary, count, index, found, kind, named, writes, empty });
    }
}

//! Real-header layout and Tier 3 parity for #3840: the original
//! obs-service-info-accessors.c.h (oracle/service_info_accessors.c) and the
//! Rust shims run the same operations on two services built from the same
//! byte pattern, through the real field adapter. After every operation the
//! results (as offsets into each service) and both services, byte for byte,
//! must match; so must the source tables, which are never written. Mutants
//! show the comparison catches a partial copy, a dropped NULL, a wrong table
//! address and a stale data read.
use core::ffi::c_void;
use core::mem::{MaybeUninit, align_of, offset_of, size_of};
use core::ptr;
use obs_output::ffi::service::ObsServiceInfo;
use obs_output::ffi::service_info_accessors::{
    Fields, obs_service_get_info_table, obs_service_get_plugin_data, obs_service_set_info,
    obs_service_set_plugin_data,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_service_set_info(service: *mut c_void, info: *const ObsServiceInfo);
    fn oracle_obs_service_set_plugin_data(service: *mut c_void, data: *mut c_void);
    fn oracle_obs_service_get_info_table(service: *const c_void) -> *const ObsServiceInfo;
    fn oracle_obs_service_get_plugin_data(service: *const c_void) -> *mut c_void;
    fn oracle_service_info_service_new(seed: u8) -> *mut c_void;
    fn oracle_service_info_service_free(service: *mut c_void);
    fn oracle_service_info_table_fill(info: *mut ObsServiceInfo, seed: u8);
    fn oracle_service_info_service_equal(a: *const c_void, b: *const c_void) -> bool;
    fn oracle_service_info_table_equal(a: *const ObsServiceInfo, b: *const ObsServiceInfo) -> bool;
    fn oracle_service_info_addresses(service: *mut c_void) -> bool;
    fn oracle_service_info_layout(layout: *mut usize);
}

/// The four accessors of one side.
#[derive(Clone, Copy)]
struct Side {
    set_info: unsafe extern "C" fn(*mut c_void, *const ObsServiceInfo),
    set_data: unsafe extern "C" fn(*mut c_void, *mut c_void),
    get_info: unsafe extern "C" fn(*const c_void) -> *const ObsServiceInfo,
    get_data: unsafe extern "C" fn(*const c_void) -> *mut c_void,
}

const C: Side = Side {
    set_info: oracle_obs_service_set_info,
    set_data: oracle_obs_service_set_plugin_data,
    get_info: oracle_obs_service_get_info_table,
    get_data: oracle_obs_service_get_plugin_data,
};

const RUST: Side = Side {
    set_info: obs_service_set_info,
    set_data: obs_service_set_plugin_data,
    get_info: obs_service_get_info_table,
    get_data: obs_service_get_plugin_data,
};

#[derive(Clone, Copy, Debug)]
enum Op {
    /// Copy a fresh table of this seed in.
    SetInfo(u8),
    /// Copy the service's own table onto itself.
    SetOwnInfo,
    /// Store a pointer into the token block (`None`: NULL).
    SetData(Option<u8>),
    GetInfo,
    GetData,
}

struct Service(*mut c_void);
impl Service {
    fn new(seed: u8) -> Self {
        // SAFETY: the oracle allocates and fills a whole struct obs_service.
        let service = unsafe { oracle_service_info_service_new(seed) };
        assert!(!service.is_null());
        Service(service)
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        // SAFETY: owns the oracle allocation.
        unsafe { oracle_service_info_service_free(self.0) }
    }
}

fn table(seed: u8) -> Box<MaybeUninit<ObsServiceInfo>> {
    let mut info = Box::new(MaybeUninit::<ObsServiceInfo>::uninit());
    // SAFETY: the oracle writes every byte of the table.
    unsafe { oracle_service_info_table_fill(info.as_mut_ptr(), seed) };
    info
}

/// Borrowed tokens the plugin data points into; never read through.
static TOKENS: [u8; 256] = [0; 256];

fn token(index: Option<u8>) -> *mut c_void {
    index.map_or(ptr::null_mut(), |i| {
        ptr::from_ref(&TOKENS[usize::from(i)]).cast_mut().cast()
    })
}

/// Runs `ops` on a C service and a candidate service built from `seed`;
/// returns the first difference.
fn compare(seed: u8, ops: &[Op], candidate: Side) -> Result<(), String> {
    let (c, r) = (Service::new(seed), Service::new(seed));
    for (step, &op) in ops.iter().enumerate() {
        let at = |what: &str| format!("step {step} {op:?}: {what}");
        // SAFETY: both services are live whole struct obs_service values;
        // each source table is filled before use and outlives the call.
        unsafe {
            match op {
                Op::SetInfo(table_seed) => {
                    let (from_c, from_r) = (table(table_seed), table(table_seed));
                    (C.set_info)(c.0, from_c.as_ptr());
                    (candidate.set_info)(r.0, from_r.as_ptr());
                    let fresh = table(table_seed);
                    if !oracle_service_info_table_equal(from_r.as_ptr(), fresh.as_ptr())
                        || !oracle_service_info_table_equal(from_c.as_ptr(), fresh.as_ptr())
                    {
                        return Err(at("source table written"));
                    }
                }
                Op::SetOwnInfo => {
                    (C.set_info)(c.0, (C.get_info)(c.0));
                    (candidate.set_info)(r.0, (RUST.get_info)(r.0));
                }
                Op::SetData(index) => {
                    (C.set_data)(c.0, token(index));
                    (candidate.set_data)(r.0, token(index));
                }
                Op::GetInfo => {
                    let from_c = (C.get_info)(c.0).byte_offset_from(c.0);
                    let from_r = (candidate.get_info)(r.0).byte_offset_from(r.0);
                    if from_c != from_r {
                        return Err(at(&format!("table at {from_r}, C {from_c}")));
                    }
                }
                Op::GetData => {
                    let (from_c, from_r) = ((C.get_data)(c.0), (candidate.get_data)(r.0));
                    if from_c != from_r {
                        return Err(at(&format!("data {from_r:?}, C {from_c:?}")));
                    }
                }
            }
            if !oracle_service_info_service_equal(c.0, r.0) {
                return Err(at("services differ"));
            }
        }
    }
    if TOKENS.iter().any(|&b| b != 0) {
        return Err("a token was written".into());
    }
    Ok(())
}

/// The sequence the unchanged C test walks, with each value read back.
fn characterized() -> Vec<Op> {
    use Op::*;
    vec![
        SetInfo(0x5a),
        GetInfo,
        GetInfo,
        SetInfo(0xc3),
        GetInfo,
        SetOwnInfo,
        GetInfo,
        SetData(Some(1)),
        GetData,
        GetData,
        SetData(None),
        GetData,
        SetData(Some(2)),
        GetData,
        SetData(Some(2)),
        GetData,
        SetInfo(0),
        GetInfo,
        GetData,
    ]
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 9];
    // SAFETY: the oracle writes exactly nine sizes, alignments and offsets.
    unsafe { oracle_service_info_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout[..6],
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, info),
            offset_of!(Fields, data),
            size_of::<ObsServiceInfo>(),
            align_of::<ObsServiceInfo>(),
        ]
    );
    // The two members sit inside the service and do not overlap.
    let (info, data, size) = (layout[6], layout[7], layout[8]);
    assert!(info + size_of::<ObsServiceInfo>() <= size && data + size_of::<usize>() <= size);
    assert!(data + size_of::<usize>() <= info || info + size_of::<ObsServiceInfo>() <= data);
    let service = Service::new(0x27);
    // SAFETY: a live whole struct obs_service.
    unsafe {
        assert!(oracle_service_info_addresses(service.0));
        assert_eq!(
            obs_service_get_info_table(service.0).byte_offset_from(service.0),
            info as isize
        );
    }
}

#[test]
fn characterized_sequence_matches_c() {
    for seed in [0x11, 0x27, 0x00, 0xff] {
        compare(seed, &characterized(), RUST).unwrap();
    }
}

unsafe extern "C" fn set_info_skips_last_word(service: *mut c_void, info: *const ObsServiceInfo) {
    // SAFETY: a live service and a readable table (shim contract).
    unsafe {
        let own = obs_service_get_info_table(service).cast_mut();
        let keep = (*own).can_try_to_connect;
        obs_service_set_info(service, info);
        (*own).can_try_to_connect = keep;
    }
}

unsafe extern "C" fn set_data_ignores_null(service: *mut c_void, data: *mut c_void) {
    if !data.is_null() {
        // SAFETY: a live service (shim contract).
        unsafe { obs_service_set_plugin_data(service, data) }
    }
}

unsafe extern "C" fn get_info_returns_service(service: *const c_void) -> *const ObsServiceInfo {
    service.cast()
}

unsafe extern "C" fn get_data_returns_null(_: *const c_void) -> *mut c_void {
    ptr::null_mut()
}

#[test]
fn mutants_are_caught() {
    let mutants = [
        Side {
            set_info: set_info_skips_last_word,
            ..RUST
        },
        Side {
            set_data: set_data_ignores_null,
            ..RUST
        },
        Side {
            get_info: get_info_returns_service,
            ..RUST
        },
        Side {
            get_data: get_data_returns_null,
            ..RUST
        },
    ];
    for (i, mutant) in mutants.into_iter().enumerate() {
        assert!(
            compare(0x11, &characterized(), mutant).is_err(),
            "mutant {i} passed"
        );
    }
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        any::<u8>().prop_map(Op::SetInfo),
        Just(Op::SetOwnInfo),
        proptest::option::of(any::<u8>()).prop_map(Op::SetData),
        Just(Op::GetInfo),
        Just(Op::GetData),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_sequences_match_c(seed in any::<u8>(), ops in proptest::collection::vec(op(), 0..24)) {
        prop_assert_eq!(compare(seed, &ops, RUST), Ok(()));
    }
}

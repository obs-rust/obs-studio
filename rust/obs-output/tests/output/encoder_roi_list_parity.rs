//! Tier 3: original C add/enumeration, real private layout and production adapter.
//! No intentional differences; generated allocations are capped at 24 ROIs.
#![allow(unsafe_code)]
use super::encoder_roi_list::{bits, rect};
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::encoder_roi::{OBS_ENCODER_CAP_ROI, Roi};
use obs_output::ffi::encoder_roi_list::{RoiListFields, obs_encoder_add_roi, obs_encoder_enum_roi};
use proptest::prelude::*;

type Add = unsafe extern "C" fn(*mut c_void, *const Roi) -> bool;
type Enum = unsafe extern "C" fn(
    *mut c_void,
    Option<unsafe extern "C" fn(*mut c_void, *mut Roi)>,
    *mut c_void,
);
unsafe extern "C" {
    fn oracle_obs_encoder_add_roi(encoder: *mut c_void, roi: *const Roi) -> bool;
    fn oracle_obs_encoder_enum_roi(
        encoder: *mut c_void,
        callback: Option<unsafe extern "C" fn(*mut c_void, *mut Roi)>,
        param: *mut c_void,
    );
    fn oracle_roi_list_new(
        caps: u32,
        sw: u32,
        sh: u32,
        w: u32,
        h: u32,
        increment: u32,
    ) -> *mut c_void;
    fn oracle_roi_list_free(encoder: *mut c_void);
    fn oracle_roi_list_state(encoder: *mut c_void, state: *mut u32);
    fn oracle_roi_list_get(encoder: *mut c_void, index: usize, roi: *mut Roi);
    fn oracle_roi_list_locked(encoder: *mut c_void) -> bool;
    fn oracle_roi_list_layout(layout: *mut usize);
    fn oracle_roi_list_null_logs(add: Add, enumerate: Enum) -> bool;
}
struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: uniquely owned fixture allocated by the matching C constructor.
        unsafe { oracle_roi_list_free(self.0) };
    }
}
impl Fixture {
    fn state(&self) -> [u32; 6] {
        let mut state = [0; 6];
        // SAFETY: live fixture and six writable u32 slots.
        unsafe { oracle_roi_list_state(self.0, state.as_mut_ptr()) };
        state
    }
    fn entries(&self) -> Vec<[u32; 5]> {
        (0..self.state()[0] as usize)
            .map(|index| {
                let mut roi = Roi::default();
                // SAFETY: index is within the live fixture's current count.
                unsafe { oracle_roi_list_get(self.0, index, &mut roi) };
                bits(&roi)
            })
            .collect()
    }
}
struct Capture {
    encoder: *mut c_void,
    seen: Vec<[u32; 5]>,
    mutate: bool,
    locked: bool,
}
unsafe extern "C" fn collect(param: *mut c_void, roi: *mut Roi) {
    // SAFETY: enum receives our live Capture and a writable current ROI.
    let capture = unsafe { &mut *param.cast::<Capture>() };
    // SAFETY: callback's fixture remains live for the entire synchronous enum.
    capture.locked &= unsafe { oracle_roi_list_locked(capture.encoder) };
    // SAFETY: enum supplies the live or temporary ROI for the callback's duration.
    let roi = unsafe { &mut *roi };
    capture.seen.push(bits(roi));
    if capture.mutate {
        roi.top = 17;
        roi.priority = -0.75;
    }
}
fn check(caps: u32, dims: [u32; 4], increment: u32, rois: &[Option<Roi>]) {
    // SAFETY: constructor initializes an independent encoder, mutex, and bmem list.
    let fixtures = [
        Fixture(unsafe {
            oracle_roi_list_new(caps, dims[0], dims[1], dims[2], dims[3], increment)
        }),
        Fixture(unsafe {
            oracle_roi_list_new(caps, dims[0], dims[1], dims[2], dims[3], increment)
        }),
    ];
    assert!(fixtures.iter().all(|f| !f.0.is_null()));
    let adds: [Add; 2] = [oracle_obs_encoder_add_roi, obs_encoder_add_roi];
    let enums: [Enum; 2] = [oracle_obs_encoder_enum_roi, obs_encoder_enum_roi];
    for roi in rois {
        let ptr = roi.as_ref().map_or(core::ptr::null(), core::ptr::from_ref);
        // SAFETY: fixtures and optional ROI remain live throughout each call.
        let result = [unsafe { adds[0](fixtures[0].0, ptr) }, unsafe {
            adds[1](fixtures[1].0, ptr)
        }];
        assert_eq!(result[0], result[1]);
        assert_eq!(fixtures[0].state(), fixtures[1].state());
        assert_eq!(fixtures[0].entries(), fixtures[1].entries());
    }
    for mutate in [false, true, false] {
        let mut captures = vec![];
        for i in 0..2 {
            let mut capture = Capture {
                encoder: fixtures[i].0,
                seen: vec![],
                mutate,
                locked: true,
            };
            // SAFETY: live fixture, function pointer and synchronous callback context.
            unsafe {
                enums[i](
                    fixtures[i].0,
                    Some(collect),
                    core::ptr::from_mut(&mut capture).cast(),
                )
            };
            assert!(capture.locked);
            captures.push(capture.seen);
        }
        assert_eq!(captures[0], captures[1]);
        assert_eq!(fixtures[0].state(), fixtures[1].state());
        assert_eq!(fixtures[0].entries(), fixtures[1].entries());
        assert_eq!(fixtures[0].state()[5], 1);
    }
}
#[test]
fn layout_matches_real_header() {
    let mut layout = [usize::MAX; 12];
    // SAFETY: C writes exactly 12 size_t values.
    unsafe { oracle_roi_list_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<RoiListFields>(),
            align_of::<RoiListFields>(),
            offset_of!(RoiListFields, rois),
            offset_of!(RoiListFields, increment),
            offset_of!(RoiListFields, caps),
            offset_of!(RoiListFields, scaled_width),
            offset_of!(RoiListFields, scaled_height),
            offset_of!(RoiListFields, media),
            offset_of!(RoiListFields, mutex),
            offset_of!(RoiListFields, lock),
            offset_of!(RoiListFields, unlock),
            offset_of!(RoiListFields, video_size)
        ]
    );
}
#[test]
fn exact_null_diagnostics() {
    if !super::isolated(concat!(module_path!(), "::exact_null_diagnostics")) {
        return;
    }
    // SAFETY: isolated child exclusively owns the global logger; both callbacks accept NULL.
    unsafe {
        assert!(oracle_roi_list_null_logs(
            oracle_obs_encoder_add_roi,
            oracle_obs_encoder_enum_roi
        ));
        assert!(oracle_roi_list_null_logs(
            obs_encoder_add_roi,
            obs_encoder_enum_roi
        ));
    }
}
#[test]
fn characterized_edges() {
    let rois = [
        None,
        Some(rect(0, 16, 0, 16, 0.5)),
        Some(rect(33, 101, 7, 1001, -1.)),
        Some(rect(0, 0xffffff, 16, 0x1000000, 0.5)),
        Some(rect(0x1000000, u32::MAX, 0xffffff, u32::MAX, -0.5)),
        Some(rect(200, 100, 0, 64, 0.5)),
        Some(rect(0, 16, 0, 16, f32::NAN)),
    ];
    for caps in [0, OBS_ENCODER_CAP_ROI, u32::MAX] {
        for dims in [
            [0, 0, 0, 0],
            [1280, 0, 0, 0],
            [0, 720, 0, 0],
            [1280, 720, 0, 0],
            [1280, 720, 1920, 1080],
            [4096, 4096, 16, 16],
            [u32::MAX, u32::MAX, 1, 1],
        ] {
            for increment in [0, u32::MAX] {
                check(caps, dims, increment, &rois);
            }
        }
    }
    check(OBS_ENCODER_CAP_ROI, [0; 4], 0, &[]);
}
fn roi_strategy() -> impl Strategy<Value = Option<Roi>> {
    prop_oneof![
        Just(None),
        (
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>()
        )
            .prop_map(|(top, bottom, left, right, p)| Some(rect(
                top,
                bottom,
                left,
                right,
                f32::from_bits(p)
            ))),
        (
            0u32..10000,
            16u32..4096,
            0u32..10000,
            16u32..4096,
            -1.0f32..=1.0
        )
            .prop_map(|(top, h, left, w, p)| Some(rect(top, top + h, left, left + w, p)))
    ]
}
proptest! {
    #![proptest_config(ProptestConfig { cases:obs_c_oracle::parity::cases(256), failure_persistence:None,..ProptestConfig::default() })]
    #[test]
    fn random_lists_match_c(caps in prop_oneof![Just(OBS_ENCODER_CAP_ROI),any::<u32>()],
        sw in prop_oneof![Just(0),1u32..8192],sh in prop_oneof![Just(0),1u32..8192],
        w in prop_oneof![Just(0),1u32..8192],h in prop_oneof![Just(0),1u32..8192],
        increment in any::<u32>(),rois in prop::collection::vec(roi_strategy(),0..24)) {
        check(caps,[sw,sh,w,h],increment,&rois);
    }
}

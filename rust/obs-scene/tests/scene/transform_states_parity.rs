//! Differential tests execute the unmodified extracted C policy. The bounded
//! scene collaborators model public API effects; unchanged Tier 2 tests use OBS.
use super::transform_states::Model;
use obs_data::data::Data;
use obs_scene::ffi::transform_states::TransformInfo;
use proptest::prelude::*;
use std::ffi::{CStr, c_char};
use std::mem::{align_of, offset_of, size_of};
use std::sync::Mutex;
static ORACLE: Mutex<()> = Mutex::new(());
unsafe extern "C" {
    fn oracle_sts_run(
        all: bool,
        groups: bool,
        selected: u8,
        bounds: i64,
        bad_id: bool,
        bad_uuid: bool,
        seed: u32,
        json: *mut c_char,
        capacity: usize,
        out: *mut u64,
    );
    fn oracle_sts_layout(out: *mut usize);
}
fn check(flags: u8, selected: u8, bounds: i64, seed: u32) {
    let _guard = ORACLE.lock().unwrap_or_else(|e| e.into_inner());
    let all = flags & 1 != 0;
    let groups = flags & 2 != 0;
    let bad_id = flags & 4 != 0;
    let bad_uuid = flags & 8 != 0;
    let mut json = vec![0u8; 16384];
    let mut expected = [0u64; 128];
    // SAFETY: buffers have the fixed oracle capacities; its fixture is mutex-protected.
    unsafe {
        oracle_sts_run(
            all,
            groups,
            selected,
            bounds,
            bad_id,
            bad_uuid,
            seed,
            json.as_mut_ptr().cast(),
            json.len(),
            expected.as_mut_ptr(),
        );
    }
    // SAFETY: snprintf terminates the nonempty buffer, whose entire allocation is readable.
    let json = unsafe { CStr::from_ptr(json.as_ptr().cast()) }
        .to_str()
        .unwrap();
    let expected_snapshot = Data::from_json(json).unwrap();
    let mut m = Model::new(groups);
    for i in 1..m.items.len() {
        m.configure(i);
        m.items[i].selected = selected & (1 << (i - 1)) != 0;
        m.items[i].info.pos[0] += (seed % 32) as f32;
        m.items[i].info.rot += (seed % 64) as f32;
    }
    let snapshot = m.snapshot(all);
    assert_eq!(
        snapshot.json(),
        expected_snapshot.json(),
        "snapshot flags={flags}, selected={selected}"
    );
    let records = snapshot.get_array("scenes_and_groups").unwrap();
    let root = records.item(records.count() - 1).unwrap();
    for item in root.get_array("items").unwrap().elements() {
        item.set_int("bounds_type", bounds);
        if bad_id {
            item.set_int("id", i64::MAX);
        }
    }
    if bad_uuid {
        root.set_string("scene_uuid", Some("missing scene uuid"));
    }
    if groups {
        records
            .item(1)
            .unwrap()
            .set_string("scene_uuid", Some("missing group uuid"));
    }
    m.clear_transforms();
    m.items[1].deferred = seed & 1;
    m.sources[1].name = "renamed scene".into();
    m.restore(&snapshot);
    let mut actual = [0u64; 128];
    for (i, v) in m.items.iter().enumerate().skip(1) {
        let t = &v.info;
        let o = &mut actual[(i - 1) * 18..i * 18];
        o[..11].copy_from_slice(&[
            u64::from(t.pos[0].to_bits()),
            u64::from(t.pos[1].to_bits()),
            u64::from(t.rot.to_bits()),
            u64::from(t.scale[0].to_bits()),
            u64::from(t.scale[1].to_bits()),
            u64::from(t.alignment),
            u64::from(t.bounds_type),
            u64::from(t.bounds_alignment),
            u64::from(t.bounds[0].to_bits()),
            u64::from(t.bounds[1].to_bits()),
            u64::from(t.crop_to_bounds),
        ]);
        for (o, c) in o[11..15].iter_mut().zip(v.crop) {
            *o = c as u64;
        }
        o[15] = u64::from(v.deferred);
        o[16] = u64::from(v.signals);
        o[17] = u64::from(v.selected);
    }
    for (i, s) in m.sources.iter().enumerate() {
        actual[108 + i] = s.refs as u64;
    }
    assert_eq!(
        actual, expected,
        "restore flags={flags}, selected={selected}, bounds={bounds}"
    );
    assert_eq!(m.live_objects(), 0);
}
#[test]
fn characterized_edges() {
    for bounds in [i64::MIN, -1, 0, 1, 2, 3, 4, 5, 6, 7, i64::MAX] {
        for flags in 0..16 {
            for selected in [0, 1, 2, 16, 32, 63] {
                check(flags, selected, bounds, 1);
            }
        }
    }
}
#[test]
fn public_struct_layouts_match_c() {
    let mut measured = [0usize; 20];
    // SAFETY: the oracle writes exactly twenty size/alignment/offset measurements.
    unsafe {
        oracle_sts_layout(measured.as_mut_ptr());
    }
    assert_eq!(
        measured,
        [
            size_of::<TransformInfo>(),
            align_of::<TransformInfo>(),
            offset_of!(TransformInfo, pos),
            offset_of!(TransformInfo, rot),
            offset_of!(TransformInfo, scale),
            offset_of!(TransformInfo, alignment),
            offset_of!(TransformInfo, bounds_type),
            offset_of!(TransformInfo, bounds_alignment),
            offset_of!(TransformInfo, bounds),
            offset_of!(TransformInfo, crop_to_bounds),
            size_of::<[i32; 4]>(),
            align_of::<[i32; 4]>(),
            0,
            4,
            8,
            12,
            size_of::<[f32; 2]>(),
            align_of::<[f32; 2]>(),
            0,
            4
        ]
    );
}
proptest! {
    #![proptest_config(ProptestConfig { cases:obs_c_oracle::parity::cases(256),failure_persistence:None,..ProptestConfig::default() })]
    #[test]
    fn snapshot_restore_matches_c(flags in 0u8..16, selected in any::<u8>(), bounds in any::<i64>(), seed in any::<u32>()) {check(flags,selected,bounds,seed);}
}

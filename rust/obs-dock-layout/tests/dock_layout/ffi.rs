//! The C ABI the frontend calls (#5230).

use std::ffi::{CStr, CString, c_char};

use obs_dock_layout::ffi::{
    OBS_DOCK_LAYOUT_BOTTOM, OBS_DOCK_LAYOUT_CHANGE, OBS_DOCK_LAYOUT_HORIZONTAL,
    OBS_DOCK_LAYOUT_RESTORED, obs_dock_layout_create, obs_dock_layout_destroy,
    obs_dock_layout_dock, obs_dock_layout_free, obs_dock_layout_observe, obs_dock_layout_record,
    obs_dock_layout_resize_axis, obs_dock_layout_resize_count, obs_dock_layout_resize_docks,
    obs_dock_layout_resize_name, obs_dock_layout_resize_size, obs_dock_layout_saves_restored_blob,
    obs_dock_layout_start, obs_dock_layout_window,
};

fn window(width: i32) -> obs_dock_layout_window {
    obs_dock_layout_window {
        width,
        height: 900,
        visible: true,
        central_x: 0,
        central_y: 0,
        central_width: width,
        central_height: 696,
        central_min_width: 100,
        central_min_height: 100,
    }
}

fn row(names: &[CString], widths: &[i32], tabs: &[*const c_char]) -> Vec<obs_dock_layout_dock> {
    let mut x = 0;
    names
        .iter()
        .zip(widths)
        .map(|(name, &width)| {
            let d = obs_dock_layout_dock {
                name: name.as_ptr(),
                area: OBS_DOCK_LAYOUT_BOTTOM,
                floating: false,
                shown: true,
                x,
                y: 700,
                width,
                height: 200,
                min_width: 150,
                min_height: 100,
                tabs: tabs.as_ptr(),
                tab_count: tabs.len(),
            };
            x += width + 4;
            d
        })
        .collect()
}

/// A string from obs_dock_layout_record, freed.
fn take(raw: *mut c_char) -> String {
    assert!(!raw.is_null());
    // SAFETY: NUL-terminated and still ours.
    let text = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_owned();
    // SAFETY: from obs_dock_layout_record, freed once.
    unsafe { obs_dock_layout_free(raw) };
    text
}

#[test]
fn a_session_through_the_c_abi() {
    let names = [
        c"scenes".to_owned(),
        c"sources".to_owned(),
        c"mixer".to_owned(),
    ];
    let record = c"v1;h1;aB=200;dmixer=396;dscenes=597;dsources=595";
    let layout = obs_dock_layout_create();
    // SAFETY: a live layout and C strings, through the whole test.
    unsafe {
        assert!(obs_dock_layout_start(
            layout,
            record.as_ptr(),
            c"h1".as_ptr()
        ));
        let full = row(&names, &[597, 595, 396], &[]);
        assert!(!obs_dock_layout_observe(
            layout,
            &window(1600),
            full.as_ptr(),
            3,
            OBS_DOCK_LAYOUT_RESTORED
        ));
        assert_eq!(obs_dock_layout_resize_count(layout), 0);
        let equal = row(&names, &[529, 529, 530], &[]);
        assert!(!obs_dock_layout_observe(
            layout,
            &window(1600),
            equal.as_ptr(),
            3,
            OBS_DOCK_LAYOUT_CHANGE
        ));
        assert_eq!(obs_dock_layout_resize_count(layout), 1);
        assert_eq!(
            obs_dock_layout_resize_axis(layout, 0),
            OBS_DOCK_LAYOUT_HORIZONTAL
        );
        assert_eq!(obs_dock_layout_resize_docks(layout, 0), 3);
        let first = CStr::from_ptr(obs_dock_layout_resize_name(layout, 0, 0));
        assert_eq!(first, c"scenes");
        let sizes: Vec<i32> = (0..3)
            .map(|j| obs_dock_layout_resize_size(layout, 0, j))
            .collect();
        assert_eq!(sizes, vec![597, 595, 396]);
        // Out of range: empty answers, no crash.
        assert_eq!(obs_dock_layout_resize_axis(layout, 1), 0);
        assert_eq!(obs_dock_layout_resize_docks(layout, 1), 0);
        assert!(obs_dock_layout_resize_name(layout, 0, 3).is_null());
        assert_eq!(obs_dock_layout_resize_size(layout, 0, 3), 0);
        assert!(!obs_dock_layout_saves_restored_blob(layout));
        assert_eq!(
            take(obs_dock_layout_record(layout, c"h2".as_ptr())),
            "v1;h2;aB=200;dmixer=396;dscenes=597;dsources=595"
        );
        obs_dock_layout_destroy(layout);
    }
}

#[test]
fn no_record_and_tab_groups_cross_the_abi() {
    let names = [c"scenes".to_owned(), c"sources".to_owned()];
    let other = c"audio";
    let tabs = [other.as_ptr()];
    let layout = obs_dock_layout_create();
    // SAFETY: as above.
    unsafe {
        assert!(!obs_dock_layout_start(
            layout,
            std::ptr::null(),
            c"h1".as_ptr()
        ));
        let docks = row(&names, &[400, 400], &tabs);
        obs_dock_layout_observe(
            layout,
            &window(808),
            docks.as_ptr(),
            2,
            OBS_DOCK_LAYOUT_RESTORED,
        );
        assert!(!obs_dock_layout_saves_restored_blob(layout));
        assert_eq!(
            take(obs_dock_layout_record(layout, c"h1".as_ptr())),
            "v1;h1;aB=200;dscenes=400;dsources=400"
        );
        obs_dock_layout_destroy(layout);
    }
}

#[test]
fn null_arguments_are_safe() {
    // SAFETY: null is allowed everywhere.
    unsafe {
        assert!(!obs_dock_layout_start(
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null()
        ));
        assert!(!obs_dock_layout_observe(
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            0
        ));
        assert_eq!(obs_dock_layout_resize_count(std::ptr::null()), 0);
        assert!(obs_dock_layout_record(std::ptr::null(), std::ptr::null()).is_null());
        assert!(!obs_dock_layout_saves_restored_blob(std::ptr::null()));
        obs_dock_layout_free(std::ptr::null_mut());
        obs_dock_layout_destroy(std::ptr::null_mut());
        let layout = obs_dock_layout_create();
        assert!(!obs_dock_layout_observe(
            layout,
            std::ptr::null(),
            std::ptr::null(),
            0,
            0
        ));
        obs_dock_layout_start(layout, std::ptr::null(), std::ptr::null());
        assert!(!obs_dock_layout_observe(
            layout,
            &window(800),
            std::ptr::null(),
            0,
            OBS_DOCK_LAYOUT_RESTORED
        ));
        assert_eq!(
            take(obs_dock_layout_record(layout, std::ptr::null())),
            "v1;"
        );
        obs_dock_layout_destroy(layout);
    }
}

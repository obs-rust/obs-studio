//! C ABI for the frontend (#5230), declared in
//! `frontend/utility/obs-dock-layout.h`. It converts types and calls the
//! safe model; there is no logic here. One model per main window, used from
//! the UI thread.

use std::ffi::{CStr, CString, c_char, c_int};

use crate::{Area, Axis, Cause, Dock, Model, Output, Rect, Snapshot};

/// Dock areas, the values of `Qt::DockWidgetArea`; anything else is none.
pub const OBS_DOCK_LAYOUT_LEFT: c_int = 0x1;
pub const OBS_DOCK_LAYOUT_RIGHT: c_int = 0x2;
pub const OBS_DOCK_LAYOUT_TOP: c_int = 0x4;
pub const OBS_DOCK_LAYOUT_BOTTOM: c_int = 0x8;

/// Axes, the values of `Qt::Orientation`.
pub const OBS_DOCK_LAYOUT_HORIZONTAL: c_int = 0x1;
pub const OBS_DOCK_LAYOUT_VERTICAL: c_int = 0x2;

/// [`Cause`] values; anything else is [`Cause::Change`].
pub const OBS_DOCK_LAYOUT_CHANGE: c_int = 0;
pub const OBS_DOCK_LAYOUT_SEPARATOR_DRAG: c_int = 1;
pub const OBS_DOCK_LAYOUT_RESTORED: c_int = 2;

/// One dock (`struct obs_dock_layout_dock`).
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_dock_layout_dock {
    pub name: *const c_char,
    pub area: c_int,
    pub floating: bool,
    pub shown: bool,
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
    pub min_width: c_int,
    pub min_height: c_int,
    /// The other docks of its tab group; null when `tab_count` is 0.
    pub tabs: *const *const c_char,
    pub tab_count: usize,
}

/// The main window (`struct obs_dock_layout_window`).
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_dock_layout_window {
    pub width: c_int,
    pub height: c_int,
    pub visible: bool,
    /// The central widget's geometry and minimum size.
    pub central_x: c_int,
    pub central_y: c_int,
    pub central_width: c_int,
    pub central_height: c_int,
    pub central_min_width: c_int,
    pub central_min_height: c_int,
}

/// A model and the output of its last snapshot.
#[allow(non_camel_case_types)]
pub struct obs_dock_layout {
    model: Model,
    output: Output,
    names: Vec<Vec<CString>>,
}

/// A C string as `String`; null is empty, invalid UTF-8 is replaced.
///
/// # Safety
/// `s` is null or a NUL-terminated string.
unsafe fn text(s: *const c_char) -> String {
    if s.is_null() {
        return String::new();
    }
    // SAFETY: the caller passes a NUL-terminated string.
    unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()
}

fn area(value: c_int) -> Option<Area> {
    match value {
        OBS_DOCK_LAYOUT_LEFT => Some(Area::Left),
        OBS_DOCK_LAYOUT_RIGHT => Some(Area::Right),
        OBS_DOCK_LAYOUT_TOP => Some(Area::Top),
        OBS_DOCK_LAYOUT_BOTTOM => Some(Area::Bottom),
        _ => None,
    }
}

/// A dock from C.
///
/// # Safety
/// As [`obs_dock_layout_observe`] for one element.
unsafe fn dock(raw: &obs_dock_layout_dock) -> Dock {
    let tabs = if raw.tabs.is_null() || raw.tab_count == 0 {
        Vec::new()
    } else {
        // SAFETY: `tabs` points to `tab_count` C strings.
        let names = unsafe { std::slice::from_raw_parts(raw.tabs, raw.tab_count) };
        // SAFETY: each is null or a C string.
        let mut tabs: Vec<String> = names.iter().map(|&n| unsafe { text(n) }).collect();
        tabs.sort();
        tabs
    };
    Dock {
        // SAFETY: null or a C string.
        name: unsafe { text(raw.name) },
        area: area(raw.area),
        floating: raw.floating,
        shown: raw.shown,
        rect: Rect {
            x: raw.x,
            y: raw.y,
            width: raw.width,
            height: raw.height,
        },
        min_width: raw.min_width,
        min_height: raw.min_height,
        tabs,
    }
}

/// A new model. Free it with [`obs_dock_layout_destroy`].
#[unsafe(no_mangle)]
pub extern "C" fn obs_dock_layout_create() -> *mut obs_dock_layout {
    Box::into_raw(Box::new(obs_dock_layout {
        model: Model::new(),
        output: Output::default(),
        names: Vec::new(),
    }))
}

/// # Safety
/// `layout` is null or from [`obs_dock_layout_create`], destroyed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_dock_layout_destroy(layout: *mut obs_dock_layout) {
    if !layout.is_null() {
        // SAFETY: from Box::into_raw, as the caller promises.
        drop(unsafe { Box::from_raw(layout) });
    }
}

/// [`Model::start`]. `record` may be null (no record).
///
/// # Safety
/// `layout` is from [`obs_dock_layout_create`]; the strings are null or
/// NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_dock_layout_start(
    layout: *mut obs_dock_layout,
    record: *const c_char,
    blob_hash: *const c_char,
) -> bool {
    // SAFETY: as the caller promises.
    let Some(layout) = (unsafe { layout.as_mut() }) else {
        return false;
    };
    // SAFETY: null or a C string.
    let record = (!record.is_null()).then(|| unsafe { text(record) });
    // SAFETY: null or a C string.
    let hash = unsafe { text(blob_hash) };
    layout.output = Output::default();
    layout.names.clear();
    layout.model.start(record.as_deref(), &hash)
}

/// [`Model::observe`]. Returns [`Output::restore_again`]; the resizes are
/// read with `obs_dock_layout_resize_*` until the next call.
///
/// # Safety
/// `layout` is from [`obs_dock_layout_create`]; `window` is null or
/// valid; `docks` points to `count` docks (null when `count` is 0) whose
/// strings are null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_dock_layout_observe(
    layout: *mut obs_dock_layout,
    window: *const obs_dock_layout_window,
    docks: *const obs_dock_layout_dock,
    count: usize,
    cause: c_int,
) -> bool {
    // SAFETY: as the caller promises.
    let (Some(layout), Some(window)) = (unsafe { layout.as_mut() }, unsafe { window.as_ref() })
    else {
        return false;
    };
    let raw = if docks.is_null() || count == 0 {
        &[][..]
    } else {
        // SAFETY: `docks` points to `count` docks.
        unsafe { std::slice::from_raw_parts(docks, count) }
    };
    let snap = Snapshot {
        width: window.width,
        height: window.height,
        visible: window.visible,
        central: Rect {
            x: window.central_x,
            y: window.central_y,
            width: window.central_width,
            height: window.central_height,
        },
        central_min_width: window.central_min_width,
        central_min_height: window.central_min_height,
        // SAFETY: each dock's strings are null or C strings.
        docks: raw.iter().map(|d| unsafe { dock(d) }).collect(),
    };
    let cause = match cause {
        OBS_DOCK_LAYOUT_SEPARATOR_DRAG => Cause::SeparatorDrag,
        OBS_DOCK_LAYOUT_RESTORED => Cause::Restored,
        _ => Cause::Change,
    };
    layout.output = layout.model.observe(&snap, cause);
    layout.names = layout
        .output
        .resizes
        .iter()
        .map(|r| {
            r.docks
                .iter()
                .map(|n| CString::new(n.replace('\0', "")).unwrap_or_default())
                .collect()
        })
        .collect();
    layout.output.restore_again
}

/// The number of resizes of the last snapshot.
///
/// # Safety
/// `layout` is null or from [`obs_dock_layout_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_dock_layout_resize_count(layout: *const obs_dock_layout) -> usize {
    // SAFETY: as the caller promises.
    unsafe { layout.as_ref() }.map_or(0, |l| l.output.resizes.len())
}

/// The axis of resize `i` (`OBS_DOCK_LAYOUT_HORIZONTAL` or `_VERTICAL`),
/// 0 when there is none.
///
/// # Safety
/// `layout` is null or from [`obs_dock_layout_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_dock_layout_resize_axis(
    layout: *const obs_dock_layout,
    i: usize,
) -> c_int {
    // SAFETY: as the caller promises.
    let resize = unsafe { layout.as_ref() }.and_then(|l| l.output.resizes.get(i));
    match resize.map(|r| r.axis) {
        Some(Axis::Horizontal) => OBS_DOCK_LAYOUT_HORIZONTAL,
        Some(Axis::Vertical) => OBS_DOCK_LAYOUT_VERTICAL,
        None => 0,
    }
}

/// The number of docks in resize `i`.
///
/// # Safety
/// `layout` is null or from [`obs_dock_layout_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_dock_layout_resize_docks(
    layout: *const obs_dock_layout,
    i: usize,
) -> usize {
    // SAFETY: as the caller promises.
    unsafe { layout.as_ref() }
        .and_then(|l| l.output.resizes.get(i))
        .map_or(0, |r| r.docks.len())
}

/// The object name of dock `j` of resize `i`, null when there is none. It
/// stays valid until the next call that changes `layout`.
///
/// # Safety
/// `layout` is null or from [`obs_dock_layout_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_dock_layout_resize_name(
    layout: *const obs_dock_layout,
    i: usize,
    j: usize,
) -> *const c_char {
    // SAFETY: as the caller promises.
    unsafe { layout.as_ref() }
        .and_then(|l| l.names.get(i))
        .and_then(|names| names.get(j))
        .map_or(std::ptr::null(), |n| n.as_ptr())
}

/// The size of dock `j` of resize `i`, 0 when there is none.
///
/// # Safety
/// `layout` is null or from [`obs_dock_layout_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_dock_layout_resize_size(
    layout: *const obs_dock_layout,
    i: usize,
    j: usize,
) -> c_int {
    // SAFETY: as the caller promises.
    unsafe { layout.as_ref() }
        .and_then(|l| l.output.resizes.get(i))
        .and_then(|r| r.sizes.get(j))
        .copied()
        .unwrap_or(0)
}

/// [`Model::saves_restored_blob`].
///
/// # Safety
/// `layout` is null or from [`obs_dock_layout_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_dock_layout_saves_restored_blob(
    layout: *const obs_dock_layout,
) -> bool {
    // SAFETY: as the caller promises.
    unsafe { layout.as_ref() }.is_some_and(|l| l.model.saves_restored_blob())
}

/// [`Model::record`]. Free it with [`obs_dock_layout_free`]; null for a
/// null `layout`.
///
/// # Safety
/// `layout` is null or from [`obs_dock_layout_create`]; `blob_hash` is null
/// or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_dock_layout_record(
    layout: *const obs_dock_layout,
    blob_hash: *const c_char,
) -> *mut c_char {
    // SAFETY: as the caller promises.
    let Some(layout) = (unsafe { layout.as_ref() }) else {
        return std::ptr::null_mut();
    };
    // SAFETY: null or a C string.
    let hash = unsafe { text(blob_hash) };
    let record = layout.model.record(&hash).replace('\0', "");
    CString::new(record).unwrap_or_default().into_raw()
}

/// Frees a string from [`obs_dock_layout_record`].
///
/// # Safety
/// `s` is null or from [`obs_dock_layout_record`], freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_dock_layout_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: from CString::into_raw, as the caller promises.
        drop(unsafe { CString::from_raw(s) });
    }
}

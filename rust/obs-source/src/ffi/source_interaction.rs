//! Thin ABI conversion; source ownership and callback lifetimes stay with C.
use crate::interaction;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MouseEvent {
    pub modifiers: u32,
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KeyEvent {
    pub modifiers: u32,
    pub text: *mut c_char,
    pub native_modifiers: u32,
    pub native_scancode: u32,
    pub native_vkey: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Fields {
    pub flags: u32,
    pub data: *mut c_void,
    pub mouse_click: Option<unsafe extern "C" fn(*mut c_void, *const MouseEvent, i32, bool, u32)>,
    pub mouse_move: Option<unsafe extern "C" fn(*mut c_void, *const MouseEvent, bool)>,
    pub mouse_wheel: Option<unsafe extern "C" fn(*mut c_void, *const MouseEvent, c_int, c_int)>,
    pub focus: Option<unsafe extern "C" fn(*mut c_void, bool)>,
    pub key_click: Option<unsafe extern "C" fn(*mut c_void, *const KeyEvent, bool)>,
}

unsafe extern "C" {
    fn obs_source_interaction_fields(source: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

unsafe fn fields(source: *const c_void) -> Option<Fields> {
    if source.is_null() {
        return None;
    }
    // SAFETY: null is rejected before the C adapter accesses any members.
    // The caller keeps the source live under the existing interaction contract.
    Some(unsafe { obs_source_interaction_fields(source) })
}

fn log(name: &'static CStr) {
    // SAFETY: the format and function name are static, terminated strings.
    unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null source and its callbacks must remain live for this call. Event
/// pointers are borrowed and forwarded unchanged, including null pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_send_mouse_click(
    source: *mut c_void,
    event: *const MouseEvent,
    kind: i32,
    up: bool,
    count: u32,
) {
    // SAFETY: inherited source lifetime; fields rejects null before access.
    let source = unsafe { fields(source) }.map(|f| {
        (
            f.flags,
            f.mouse_click.map(|callback| {
                move |(event, kind, up, count)| {
                    // SAFETY: forward the original data and payload under the C callback contract.
                    unsafe { callback(f.data, event, kind, up, count) }
                }
            }),
        )
    });
    interaction::dispatch(
        source,
        (event, kind, up, count),
        c"obs_source_send_mouse_click",
        log,
    );
}

/// # Safety
/// A non-null source and its callbacks must remain live for this call. Event
/// pointers are borrowed and forwarded unchanged, including null pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_send_mouse_move(
    source: *mut c_void,
    event: *const MouseEvent,
    leave: bool,
) {
    // SAFETY: inherited source lifetime; fields rejects null before access.
    let source = unsafe { fields(source) }.map(|f| {
        (
            f.flags,
            f.mouse_move.map(|callback| {
                move |(event, leave)| {
                    // SAFETY: forward the original data and payload under the C callback contract.
                    unsafe { callback(f.data, event, leave) }
                }
            }),
        )
    });
    interaction::dispatch(source, (event, leave), c"obs_source_send_mouse_move", log);
}

/// # Safety
/// A non-null source and its callbacks must remain live for this call. Event
/// pointers are borrowed and forwarded unchanged, including null pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_send_mouse_wheel(
    source: *mut c_void,
    event: *const MouseEvent,
    x: c_int,
    y: c_int,
) {
    // SAFETY: inherited source lifetime; fields rejects null before access.
    let source = unsafe { fields(source) }.map(|f| {
        (
            f.flags,
            f.mouse_wheel.map(|callback| {
                move |(event, x, y)| {
                    // SAFETY: forward the original data and payload under the C callback contract.
                    unsafe { callback(f.data, event, x, y) }
                }
            }),
        )
    });
    interaction::dispatch(source, (event, x, y), c"obs_source_send_mouse_wheel", log);
}

/// # Safety
/// A non-null source and its callbacks must remain live for this call. Event
/// pointers are borrowed and forwarded unchanged, including null pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_send_focus(source: *mut c_void, focus: bool) {
    // SAFETY: inherited source lifetime; fields rejects null before access.
    let source = unsafe { fields(source) }.map(|f| {
        (
            f.flags,
            f.focus.map(|callback| {
                move |(focus,)| {
                    // SAFETY: forward the original data and payload under the C callback contract.
                    unsafe { callback(f.data, focus) }
                }
            }),
        )
    });
    interaction::dispatch(source, (focus,), c"obs_source_send_focus", log);
}

/// # Safety
/// A non-null source and its callbacks must remain live for this call. Event
/// pointers are borrowed and forwarded unchanged, including null pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_send_key_click(
    source: *mut c_void,
    event: *const KeyEvent,
    up: bool,
) {
    // SAFETY: inherited source lifetime; fields rejects null before access.
    let source = unsafe { fields(source) }.map(|f| {
        (
            f.flags,
            f.key_click.map(|callback| {
                move |(event, up)| {
                    // SAFETY: forward the original data and payload under the C callback contract.
                    unsafe { callback(f.data, event, up) }
                }
            }),
        )
    });
    interaction::dispatch(source, (event, up), c"obs_source_send_key_click", log);
}

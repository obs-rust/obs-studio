//! Borrowed native fields; the private source layout stays owned by C.
use crate::base_size::{self, Axis, Source};
use crate::size_route::Node;
use core::ffi::{c_char, c_int, c_long, c_void};
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};

type SizeCallback = Option<unsafe extern "C" fn(*mut c_void) -> u32>;
#[repr(C)]
pub struct BaseSizeFields {
    pub source_type: *const c_int,
    pub data: *const *mut c_void,
    pub get_width: *const SizeCallback,
    pub get_height: *const SizeCallback,
    pub filter_parent: *const *mut c_void,
    pub filter_target: *const *mut c_void,
    pub enabled: *const bool,
    pub async_active: *const bool,
    pub async_rotation: *const c_long,
    pub transition_width: *const c_long,
    pub transition_height: *const c_long,
    pub async_width: *const u32,
    pub async_height: *const u32,
}
unsafe extern "C" {
    fn obs_source_base_size_fields(source: *const c_void) -> BaseSizeFields;
    fn blog(level: c_int, format: *const c_char, ...);
}
#[derive(Clone, Copy)]
struct Native(*const c_void);
impl Native {
    fn fields(self) -> BaseSizeFields {
        // SAFETY: Native is constructed only from checked non-null live sources.
        unsafe { obs_source_base_size_fields(self.0) }
    }
}
unsafe fn atomic_long(value: *const c_long) -> c_long {
    // SAFETY: the C long field has its native alignment and remains live.
    unsafe {
        if size_of::<c_long>() == size_of::<i32>() {
            (*value.cast::<AtomicI32>()).load(Ordering::SeqCst) as c_long
        } else {
            (*value.cast::<AtomicI64>()).load(Ordering::SeqCst) as c_long
        }
    }
}
impl Source for Native {
    fn has_data(self) -> bool {
        // SAFETY: the accessor returns a readable context data field.
        unsafe { !(*self.fields().data).is_null() }
    }
    fn node(self, axis: Axis) -> Node {
        let f = self.fields();
        // SAFETY: these are readable fields of the live source. async_active is
        // loaded atomically, matching os_atomic_load_bool in the C helper.
        unsafe {
            Node {
                source_type: *f.source_type,
                has_data: !(*f.data).is_null(),
                has_callback: (*if axis == Axis::Width {
                    f.get_width
                } else {
                    f.get_height
                })
                .is_some(),
                has_filter_parent: !(*f.filter_parent).is_null(),
                enabled: *f.enabled,
                async_active: (*f.async_active.cast::<AtomicBool>()).load(Ordering::SeqCst),
                async_rotation: *f.async_rotation,
            }
        }
    }
    fn transition_size(self, axis: Axis) -> u32 {
        let f = self.fields();
        // SAFETY: the source owns the selected aligned long field.
        unsafe {
            atomic_long(if axis == Axis::Width {
                f.transition_width
            } else {
                f.transition_height
            }) as u32
        }
    }
    fn callback_size(self, axis: Axis) -> u32 {
        let f = self.fields();
        // SAFETY: route selected an existing callback with non-null context data.
        unsafe {
            (*if axis == Axis::Width {
                f.get_width
            } else {
                f.get_height
            })
            .unwrap()(*f.data)
        }
    }
    fn target(self) -> Option<Self> {
        // SAFETY: a target read follows the C caller's synchronization contract.
        let target = unsafe { *self.fields().filter_target };
        (!target.is_null()).then_some(Self(target))
    }
    fn async_size(self, axis: Axis) -> u32 {
        let f = self.fields();
        // SAFETY: the selected dimension is readable under the C caller contract.
        unsafe {
            *if axis == Axis::Width {
                f.async_width
            } else {
                f.async_height
            }
        }
    }
}
fn null_log(axis: Axis) {
    let function = if axis == Axis::Width {
        c"obs_source_get_base_width"
    } else {
        c"obs_source_get_base_height"
    };
    // SAFETY: all variadic arguments match the constant format's string slots.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            c"source".as_ptr(),
        );
    }
}
/// # Safety
/// Non-null sources and their filter targets must be live under the original C synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_base_width(source: *const c_void) -> u32 {
    base_size::query(
        (!source.is_null()).then_some(Native(source)),
        Axis::Width,
        null_log,
    )
}
/// # Safety
/// Same requirements as obs_source_get_base_width.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_base_height(source: *const c_void) -> u32 {
    base_size::query(
        (!source.is_null()).then_some(Native(source)),
        Axis::Height,
        null_log,
    )
}
/// # Safety
/// Source must be non-null and live with all traversed filter targets.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_base_width(source: *const c_void) -> u32 {
    if source.is_null() {
        return 0;
    }
    base_size::base(Native(source), Axis::Width)
}
/// # Safety
/// Same requirements as obs_source_base_width.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_base_height(source: *const c_void) -> u32 {
    if source.is_null() {
        return 0;
    }
    base_size::base(Native(source), Axis::Height)
}

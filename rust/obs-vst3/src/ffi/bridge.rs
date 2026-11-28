use super::Engine;
use super::host_app::HostApp;
use std::ffi::{CStr, c_char, c_void};
use std::path::Path;
use std::sync::Mutex;
use std::{ptr, slice};
use vst3::Steinberg::Vst::{IHostApplicationTrait, IPlugInterfaceSupportTrait, String128};
use vst3::{
    ComRef,
    Steinberg::{FUnknown, TUID},
};

pub struct RustHost {
    name: std::ffi::CString,
    sidechain_channels: usize,
    instance: Mutex<Engine>,
}

pub struct SavedState {
    component: Vec<u8>,
    controller: Vec<u8>,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_create(
    path: *const c_char,
    class_id: *const c_char,
    sample_rate: f64,
    max_frames: i32,
    arrangement: u64,
    context: *mut FUnknown,
) -> *mut RustHost {
    if path.is_null() || class_id.is_null() || context.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: Both strings are borrowed, NUL-terminated UTF-8 from the native
    // adapter. The application retains its host context through module teardown.
    let (path, class_id, context) = unsafe {
        (
            CStr::from_ptr(path).to_str(),
            CStr::from_ptr(class_id).to_str(),
            ComRef::from_raw(context),
        )
    };
    let (Ok(path), Ok(class_id), Some(context)) = (path, class_id, context) else {
        return ptr::null_mut();
    };
    let Ok(instance) = Engine::with_host(
        Path::new(path),
        class_id,
        sample_rate,
        max_frames,
        arrangement,
        context.to_com_ptr(),
    ) else {
        return ptr::null_mut();
    };
    let name = instance.name.clone();
    let sidechain_channels = instance.sidechain_channels();
    Box::into_raw(Box::new(RustHost {
        name,
        sidechain_channels,
        instance: Mutex::new(instance),
    }))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_destroy(host: *mut RustHost) {
    if !host.is_null() {
        // SAFETY: The native owner returns the single boxed handle after
        // stopping the audio callback and dropping the Qt editor.
        drop(unsafe { Box::from_raw(host) });
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_name(host: *const RustHost) -> *const c_char {
    // SAFETY: The native caller owns the live opaque host throughout this view.
    unsafe { host.as_ref() }.map_or(ptr::null(), |host| host.name.as_ptr())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_sidechain_channels(host: *const RustHost) -> usize {
    // SAFETY: The native caller owns the live opaque host during this call.
    unsafe { host.as_ref() }.map_or(0, |host| host.sidechain_channels)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_editor_create(
    host: *const RustHost,
    platform: *const c_char,
) -> *mut super::editor::Editor {
    // SAFETY: The native owner retains the processor while its Qt editor
    // borrows the thread-affine controller. The returned pointer is borrowed.
    let controller = unsafe { host.as_ref() }
        .and_then(|host| host.instance.lock().ok())
        .map_or(ptr::null_mut(), |instance| instance.controller_ptr());
    // SAFETY: The native owner keeps the live engine through editor destruction.
    unsafe { super::editor::create_editor(controller, platform) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_deactivate(host: *const RustHost) {
    // SAFETY: Native lifecycle calls borrow the opaque handle until they return.
    if let Some(mut instance) = unsafe { host.as_ref() }.and_then(|host| host.instance.lock().ok())
    {
        let _ = instance.activate(false);
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_set_processing(host: *const RustHost, processing: bool) {
    // SAFETY: Native lifecycle calls borrow the opaque handle until they return.
    if let Some(mut instance) = unsafe { host.as_ref() }.and_then(|host| host.instance.lock().ok())
    {
        let _ = instance.set_processing(processing);
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_process(
    host: *const RustHost,
    main: *const *mut f32,
    channels: usize,
    sidechain: *const *const f32,
    sidechain_frames: usize,
    frames: usize,
    timestamp: i64,
    gui_pending: *mut bool,
) -> bool {
    if main.is_null() || gui_pending.is_null() {
        return false;
    }
    // SAFETY: The native owner retains the host for the synchronous audio call.
    let Some(mut instance) =
        (unsafe { host.as_ref() }).and_then(|host| host.instance.try_lock().ok())
    else {
        return false;
    };
    if frames > instance.max_frames {
        return false;
    }
    // SAFETY: OBS supplies channels live main planes of at least frames each.
    // Borrow samples only for each copy so even aliased planes are supported.
    let main = unsafe { slice::from_raw_parts(main, channels) };
    let main_input = instance.main_input;
    for (channel, &source) in main.iter().enumerate() {
        // SAFETY: Each main plane is null or readable for frames.
        unsafe { copy_input(&mut instance, main_input, channel, source, frames, frames) };
    }
    if let Some(bus) = instance.sidechain {
        for channel in 0..instance.sidechain_channels() {
            let source = if sidechain.is_null() {
                ptr::null()
            } else {
                // SAFETY: A supplied sidechain has one entry per configured channel.
                unsafe { *sidechain.add(channel) }
            };
            // SAFETY: Each sidechain plane is null or readable for sidechain_frames.
            unsafe {
                copy_input(
                    &mut instance,
                    bus,
                    channel,
                    source,
                    sidechain_frames,
                    frames,
                )
            };
        }
    }
    let processed = instance.process(frames, timestamp).is_ok();
    // A plug-in can request a restart while reporting a failed block.
    // SAFETY: The native caller supplies writable storage for the flag.
    unsafe { gui_pending.write(instance.needs_gui_drain()) };
    if !processed {
        return false;
    }
    for (channel, &target) in main.iter().enumerate() {
        if !target.is_null()
            && let Some(source) = instance
                .output_channel(instance.main_output, channel)
                .and_then(|samples| samples.get(..frames))
        {
            // SAFETY: The non-null main plane is also writable for frames.
            unsafe { slice::from_raw_parts_mut(target, frames) }.copy_from_slice(source);
        }
    }
    true
}

unsafe fn copy_input(
    instance: &mut Engine,
    bus: usize,
    channel: usize,
    source: *const f32,
    source_frames: usize,
    frames: usize,
) {
    let Some(target) = instance
        .input_channel(bus, channel)
        .and_then(|samples| samples.get_mut(..frames))
    else {
        return;
    };
    if source.is_null() {
        target.fill(0.0);
    } else {
        let copied = frames.min(source_frames);
        // SAFETY: The caller guarantees the source plane has at least copied samples.
        target[..copied].copy_from_slice(unsafe { slice::from_raw_parts(source, copied) });
        target[copied..].fill(0.0);
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_drain_gui(host: *const RustHost) {
    // SAFETY: The caller holds its opaque host and invokes this on the GUI thread.
    if let Some(mut instance) = unsafe { host.as_ref() }.and_then(|host| host.instance.lock().ok())
    {
        instance.drain_gui_changes();
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_save(host: *const RustHost) -> *mut SavedState {
    // SAFETY: The native caller retains the host and serializes state on the GUI thread.
    let Some(instance) = (unsafe { host.as_ref() }).and_then(|host| host.instance.lock().ok())
    else {
        return ptr::null_mut();
    };
    let Ok((component, controller)) = instance.save_state() else {
        return ptr::null_mut();
    };
    Box::into_raw(Box::new(SavedState {
        component,
        controller,
    }))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_state_data(
    state: *const SavedState,
    controller: bool,
    size: *mut usize,
) -> *const u8 {
    if size.is_null() {
        return ptr::null();
    }
    // SAFETY: The native caller holds the live state handle through the copy.
    let Some(state) = (unsafe { state.as_ref() }) else {
        return ptr::null();
    };
    let data = if controller {
        &state.controller
    } else {
        &state.component
    };
    // SAFETY: The required size output points to writable caller storage.
    unsafe { size.write(data.len()) };
    data.as_ptr()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_state_destroy(state: *mut SavedState) {
    if !state.is_null() {
        // SAFETY: The native owner returns an unconsumed boxed state.
        drop(unsafe { Box::from_raw(state) });
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_restore(
    host: *const RustHost,
    component: *const u8,
    component_size: usize,
    controller: *const u8,
    controller_size: usize,
) -> bool {
    if component.is_null() || (controller.is_null() && controller_size != 0) {
        return false;
    }
    // SAFETY: The native caller retains the host through the state exchange.
    let Some(mut instance) = (unsafe { host.as_ref() }).and_then(|host| host.instance.lock().ok())
    else {
        return false;
    };
    // SAFETY: The caller supplies live state bytes (the controller is optional).
    let component = unsafe { slice::from_raw_parts(component, component_size) };
    let controller = if controller_size == 0 {
        &[]
    } else {
        // SAFETY: The nonempty controller state points to a readable byte span.
        unsafe { slice::from_raw_parts(controller, controller_size) }
    };
    instance.restore_state(component, controller).is_ok()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_app_name(name: *mut String128) -> i32 {
    // SAFETY: The native VST3 ABI guarantees its writable SDK String128 buffer.
    unsafe { HostApp.getName(name) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_app_create(
    cid: *mut TUID,
    iid: *mut TUID,
    object: *mut *mut c_void,
) -> i32 {
    // SAFETY: The native adapter forwards the SDK call's borrowed arguments.
    unsafe { HostApp.createInstance(cid, iid, object) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_host_app_supports(iid: *const TUID) -> i32 {
    // SAFETY: The native adapter forwards the SDK call's borrowed interface ID.
    unsafe { HostApp.isPlugInterfaceSupported(iid) }
}

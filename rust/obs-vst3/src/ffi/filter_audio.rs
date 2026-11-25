//! Private adapter for the C OBS plug-in registration.

use crate::filter::audio::{AuxCapture, FilterAudio};
use std::ffi::c_void;
use std::{ptr, slice};

const MAX_PLANES: usize = 8;

pub struct AudioState {
    channels: usize,
    filter: FilterAudio,
}

pub struct CaptureState {
    channels: usize,
    capture: AuxCapture,
}

type Process = unsafe extern "C" fn(
    ctx: *mut c_void,
    main: *const *mut f32,
    aux: *const *const f32,
    channels: usize,
    frames: usize,
);

#[unsafe(no_mangle)]
pub extern "C" fn vst3_filter_audio_create(
    channels: usize,
    block_frames: usize,
    latency: u64,
) -> *mut AudioState {
    if !(1..=MAX_PLANES).contains(&channels)
        || block_frames == 0
        || block_frames > isize::MAX as usize / 32 / channels
    {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(AudioState {
        channels,
        filter: FilterAudio::new(channels, block_frames, latency),
    }))
}

/// # Safety
///
/// State comes from vst3_filter_audio_create and no audio callback is active.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_audio_destroy(state: *mut AudioState) {
    if !state.is_null() {
        // SAFETY: This function consumes the original box exactly once.
        drop(unsafe { Box::from_raw(state) });
    }
}

/// # Safety
///
/// State is live and this call runs before the filter is published or with
/// exclusive access to its audio state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_capture_create(state: *const AudioState) -> *mut CaptureState {
    // SAFETY: The caller holds access to the live, immutably borrowed state.
    let Some(state) = (unsafe { state.as_ref() }) else {
        return ptr::null_mut();
    };
    Box::into_raw(Box::new(CaptureState {
        channels: state.channels,
        capture: state.filter.aux_capture(),
    }))
}

/// # Safety
///
/// State comes from vst3_filter_capture_create. The capture callback is
/// detached and OBS has returned from obs_source_remove_audio_capture_callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_capture_destroy(state: *mut CaptureState) {
    if !state.is_null() {
        // SAFETY: The native caller has stopped using the capture state.
        drop(unsafe { Box::from_raw(state) });
    }
}

/// # Safety
///
/// Capture state remains live and every nonnull plane has frames initialized
/// audio samples for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_capture_push(
    state: *const CaptureState,
    data: *const *const f32,
    frames: u32,
    timestamp: u64,
) {
    // SAFETY: Capture state is not freed by the caller while this call runs.
    let Some(state) = (unsafe { state.as_ref() }) else {
        return;
    };
    // SAFETY: The caller supplies the plane array and samples.
    if let Some(planes) = unsafe { audio_planes(data, state.channels, frames) } {
        state.capture.push(timestamp, &planes[..state.channels]);
    }
}

/// # Safety
///
/// The native caller holds a live capture handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_capture_reset_timestamp(state: *const CaptureState) {
    // SAFETY: The native caller keeps the capture alive for this call.
    if let Some(state) = unsafe { state.as_ref() } {
        state.capture.reset_timestamp();
    }
}

/// # Safety
///
/// State is live, exclusively held by the filter audio callback. Input and
/// output descriptor arrays are valid and nonoverlapping. The processor borrows
/// the temporary pointers only during its call. Returned output sample pointers
/// are valid until the next audio process call or filter state destruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_audio_process(
    state: *mut AudioState,
    input: *const *const f32,
    frames: u32,
    timestamp: u64,
    aux_enabled: bool,
    ctx: *mut c_void,
    process: Option<Process>,
    output: *mut *mut f32,
    output_frames: *mut u32,
    output_timestamp: *mut u64,
) -> bool {
    // SAFETY: This native filter callback is the sole reader/writer of AudioState.
    let Some(state) = (unsafe { state.as_mut() }) else {
        return false;
    };
    if output.is_null() || output_frames.is_null() || output_timestamp.is_null() {
        return false;
    }
    // SAFETY: The caller supplies the plane array and samples.
    let Some(planes) = (unsafe { audio_planes(input, state.channels, frames) }) else {
        return false;
    };
    let packet = state.filter.filter(
        timestamp,
        &planes[..state.channels],
        aux_enabled,
        |main, aux| {
            if let Some(process) = process {
                let main_ptrs: [*mut f32; MAX_PLANES] = std::array::from_fn(|channel| {
                    main.get_mut(channel)
                        .map_or(ptr::null_mut(), Vec::as_mut_ptr)
                });
                let aux_ptrs: [*const f32; MAX_PLANES] = std::array::from_fn(|channel| {
                    aux.and_then(|planes| planes.get(channel))
                        .map_or(ptr::null(), Vec::as_ptr)
                });
                // SAFETY: The callback only accesses the indicated initialized,
                // disjoint planes for this fixed-size block.
                unsafe {
                    process(
                        ctx,
                        main_ptrs.as_ptr(),
                        aux.map_or(ptr::null(), |_| aux_ptrs.as_ptr()),
                        main.len(),
                        main[0].len(),
                    );
                }
            }
        },
    );
    let Some(packet) = packet else { return false };
    // SAFETY: The caller supplies space for state.channels output pointers and
    // valid writable frame/timestamp slots. Plane pointers stay in the state.
    unsafe {
        let base = packet.samples.as_mut_ptr();
        for ch in 0..state.channels {
            output.add(ch).write(base.add(ch * packet.stride));
        }
        output_frames.write(packet.frames as u32);
        output_timestamp.write(packet.timestamp);
    }
    true
}

/// # Safety
///
/// The array contains channels readable pointers. Every nonnull pointer refers
/// to a readable f32 plane of at least frames elements.
unsafe fn audio_planes<'a>(
    data: *const *const f32,
    channels: usize,
    frames: u32,
) -> Option<[&'a [f32]; MAX_PLANES]> {
    let mut planes: [&[f32]; MAX_PLANES] = [&[]; MAX_PLANES];
    let frames = frames as usize;
    if frames == 0 {
        return Some(planes);
    }
    if data.is_null() || frames > isize::MAX as usize / std::mem::size_of::<f32>() {
        return None;
    }
    for (channel, plane) in planes[..channels].iter_mut().enumerate() {
        // SAFETY: The caller supplies at least channels readable pointers.
        let sample = unsafe { data.add(channel).read() };
        if sample.is_null() || !sample.is_aligned() {
            return None;
        }
        // SAFETY: The caller provides initialized samples and a live borrow;
        // byte length and alignment are checked above.
        *plane = unsafe { slice::from_raw_parts(sample, frames) };
    }
    Some(planes)
}

pub struct FilterState(Vec<u8>);

/// # Safety
///
/// A nonempty input references size initialized bytes until this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_state_transcode(
    input: *const u8,
    size: usize,
    encode: bool,
) -> *mut FilterState {
    if encode && size > isize::MAX as usize / 2 {
        return ptr::null_mut();
    }
    let input = if size == 0 {
        &[]
    } else {
        if input.is_null() || size > isize::MAX as usize {
            return ptr::null_mut();
        }
        // SAFETY: The caller provides the initialized input bytes.
        unsafe { slice::from_raw_parts(input, size) }
    };
    let result = if encode {
        crate::filter::settings::encode_state(input).into_bytes()
    } else {
        let Ok(value) = std::str::from_utf8(input) else {
            return ptr::null_mut();
        };
        let Ok(value) = crate::filter::settings::decode_state(value) else {
            return ptr::null_mut();
        };
        value
    };
    Box::into_raw(Box::new(FilterState(result)))
}

/// # Safety
///
/// State is live and size references writable storage. The returned bytes
/// remain valid until the caller destroys state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_state_bytes(
    state: *const FilterState,
    size: *mut usize,
) -> *const u8 {
    // SAFETY: The caller keeps the allocation alive.
    let Some(state) = (unsafe { state.as_ref() }) else {
        return ptr::null();
    };
    if size.is_null() {
        return ptr::null();
    }
    // SAFETY: The caller supplies writable storage for the exact data length.
    unsafe { size.write(state.0.len()) };
    state.0.as_ptr()
}

/// # Safety
///
/// State was returned by vst3_filter_state_transcode and is consumed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_state_destroy(state: *mut FilterState) {
    if !state.is_null() {
        // SAFETY: The caller passes the original Rust allocation.
        drop(unsafe { Box::from_raw(state) });
    }
}

pub struct RouteState(crate::filter::route::Route<*mut c_void>);

pub struct RouteLookup {
    lookup: crate::filter::route::Lookup,
    name: std::ffi::CString,
}

#[unsafe(no_mangle)]
pub extern "C" fn vst3_filter_route_create() -> *mut RouteState {
    Box::into_raw(Box::new(RouteState(crate::filter::route::Route::default())))
}

/// # Safety
///
/// The caller excludes other route operations and detaches any source first.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_route_destroy(state: *mut RouteState) {
    // SAFETY: OBS owns this allocation and has finished using it.
    drop(unsafe { Box::from_raw(state) });
}

/// # Safety
///
/// The caller serializes route operations. Name is a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_route_select(
    state: *mut RouteState,
    name: *const std::ffi::c_char,
) -> *mut c_void {
    // SAFETY: The caller holds exclusive access and supplies a valid name.
    let state = unsafe { &mut *state };
    let name = unsafe { std::ffi::CStr::from_ptr(name) };
    state
        .0
        .select(&name.to_string_lossy())
        .unwrap_or(ptr::null_mut())
}

/// # Safety
///
/// The caller serializes route operations and releases the returned reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_route_detach(state: *mut RouteState) -> *mut c_void {
    // SAFETY: The caller holds exclusive access.
    unsafe { &mut *state }.0.detach().unwrap_or(ptr::null_mut())
}

/// # Safety
///
/// The caller serializes route operations.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_route_lookup(
    state: *mut RouteState,
    now_ns: u64,
) -> *mut RouteLookup {
    // SAFETY: The caller holds exclusive access.
    let state = unsafe { &mut *state };
    let Some(lookup) = state.0.lookup(now_ns) else {
        return ptr::null_mut();
    };
    let Ok(name) = std::ffi::CString::new(lookup.name.as_str()) else {
        return ptr::null_mut();
    };
    Box::into_raw(Box::new(RouteLookup { lookup, name }))
}

/// # Safety
///
/// Lookup is live. The returned string borrows it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_route_name(
    lookup: *const RouteLookup,
) -> *const std::ffi::c_char {
    // SAFETY: The original lookup outlives the borrowed name.
    unsafe { &*lookup }.name.as_ptr()
}

/// # Safety
///
/// The caller serializes route operations and provides a live lookup token.
/// Source is a borrowed weak OBS source pointer. A successful call transfers
/// its reference to the route until removed by selection.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_route_attach(
    state: *mut RouteState,
    lookup: *const RouteLookup,
    source: *mut c_void,
) -> bool {
    // SAFETY: The caller holds exclusive access to state and owns lookup.
    let state = unsafe { &mut *state };
    let lookup = unsafe { &*lookup };
    state.0.attach(&lookup.lookup, source).is_ok()
}

/// # Safety
///
/// Lookup was allocated by vst3_filter_route_lookup and is unused elsewhere.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_route_lookup_destroy(lookup: *mut RouteLookup) {
    // SAFETY: Consume the unique lookup allocation.
    drop(unsafe { Box::from_raw(lookup) });
}

pub struct CatalogBuilder(Vec<crate::filter::catalog::Class>);
pub struct CatalogState(crate::filter::catalog::Catalog);

#[unsafe(no_mangle)]
pub extern "C" fn vst3_filter_catalog_begin() -> *mut CatalogBuilder {
    Box::into_raw(Box::new(CatalogBuilder(Vec::new())))
}

/// # Safety
///
/// Builder is exclusively borrowed and all strings are valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_catalog_add(
    builder: *mut CatalogBuilder,
    id: *const std::ffi::c_char,
    path: *const std::ffi::c_char,
    name: *const std::ffi::c_char,
    plugin_name: *const std::ffi::c_char,
    effect: bool,
) {
    // SAFETY: The caller holds exclusive access and supplies valid strings.
    let builder = unsafe { &mut *builder };
    let text = |value| {
        unsafe { std::ffi::CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned()
    };
    builder.0.push(crate::filter::catalog::Class {
        id: text(id),
        path: text(path),
        name: text(name),
        plugin_name: text(plugin_name),
        effect,
    });
}

/// # Safety
///
/// Builder was allocated by vst3_filter_catalog_begin and is no longer in use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_catalog_finish(
    builder: *mut CatalogBuilder,
) -> *mut CatalogState {
    // SAFETY: The caller owns the unique allocation.
    let builder = unsafe { Box::from_raw(builder) };
    let catalog =
        crate::filter::catalog::Catalog::new(builder.0, |path| std::path::Path::new(path).exists());
    Box::into_raw(Box::new(CatalogState(catalog)))
}

/// # Safety
///
/// State was allocated by vst3_filter_catalog_finish and is no longer in use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_catalog_destroy(state: *mut CatalogState) {
    // SAFETY: The caller owns the unique allocation.
    drop(unsafe { Box::from_raw(state) });
}

type TextPair = unsafe extern "C" fn(*mut c_void, *const std::ffi::c_char, *const std::ffi::c_char);

/// # Safety
///
/// State is live if nonnull and callbacks use borrowed strings only during
/// the call. Callback context points to native data valid throughout the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_catalog_choices(
    state: *const CatalogState,
    ctx: *mut c_void,
    add: TextPair,
) {
    // SAFETY: The caller keeps the catalog alive.
    let Some(state) = (unsafe { state.as_ref() }) else {
        return;
    };
    for (label, id) in state.0.choices() {
        if let (Ok(label), Ok(id)) = (std::ffi::CString::new(label), std::ffi::CString::new(id)) {
            // SAFETY: Both strings outlive the callback.
            unsafe { add(ctx, label.as_ptr(), id.as_ptr()) };
        }
    }
}

/// # Safety
///
/// State is live if nonnull. Inputs contain valid C strings. Callback context
/// and its borrowed strings are live throughout the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vst3_filter_class_change(
    state: *const CatalogState,
    current: *const std::ffi::c_char,
    has_processor: bool,
    requested: *const std::ffi::c_char,
    cached_path: *const std::ffi::c_char,
    cached_name: *const std::ffi::c_char,
    ctx: *mut c_void,
    set_location: TextPair,
) -> u8 {
    use crate::filter::settings::{Location, SelectionChange, change};
    // SAFETY: The caller has retained an immutable reference during the call.
    let state = unsafe { state.as_ref() };
    // SAFETY: The settings strings are valid for the duration of the call.
    let text = |value| unsafe { std::ffi::CStr::from_ptr(value) }.to_string_lossy();
    let current = text(current);
    let requested = text(requested);
    let path = text(cached_path);
    let name = text(cached_name);
    let cached = Location {
        path: &path,
        name: &name,
    };
    match change(&current, has_processor, &requested, cached, || {
        state.and_then(|state| state.0.location(&requested))
    }) {
        SelectionChange::Clear => 0,
        SelectionChange::Retain => 1,
        SelectionChange::Install { location, initial } => {
            if let (Ok(path), Ok(name)) = (
                std::ffi::CString::new(location.path),
                std::ffi::CString::new(location.name),
            ) {
                // SAFETY: Both strings outlive the callback.
                unsafe { set_location(ctx, path.as_ptr(), name.as_ptr()) };
            }
            if initial { 2 } else { 3 }
        }
    }
}

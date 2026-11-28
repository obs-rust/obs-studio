use std::cell::RefCell;
use std::ffi::{c_char, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};

#[cfg(target_os = "linux")]
use vst3::Steinberg::Linux::{IRunLoop, IRunLoop_iid};
use vst3::Steinberg::Vst::{IEditController, IEditControllerTrait, ViewType};
use vst3::Steinberg::{
    FUnknown, FUnknown_iid, FUnknownVtbl, IPlugFrame, IPlugFrame_iid, IPlugFrameVtbl, IPlugView,
    IPlugViewTrait, TUID, ViewRect, kInvalidArgument, kNoInterface, kResultFalse, kResultOk,
    kResultTrue, tresult,
};
use vst3::{ComPtr, ComRef};

type Resize = unsafe extern "C" fn(*mut c_void, *const ViewRect) -> tresult;

#[derive(Clone, Copy)]
struct Window {
    context: usize,
    resize: Resize,
}

#[repr(C)]
struct Frame {
    interface: IPlugFrame,
    ui_thread: ThreadId,
    view: AtomicPtr<IPlugView>,
    window: Mutex<Option<Window>>,
    resizing: AtomicBool,
    #[cfg(target_os = "linux")]
    run_loop: Option<ComPtr<IRunLoop>>,
}

impl Frame {
    const VTABLE: IPlugFrameVtbl = IPlugFrameVtbl {
        base: FUnknownVtbl {
            queryInterface: Self::query_interface,
            addRef: Self::add_ref,
            release: Self::release,
        },
        resizeView: Self::resize_view,
    };

    unsafe extern "system" fn query_interface(
        this: *mut FUnknown,
        iid: *const TUID,
        object: *mut *mut c_void,
    ) -> tresult {
        if iid.is_null() || object.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: COM callers supply live parameters for this call.
        let (frame, iid, object) = unsafe { (&*this.cast::<Self>(), &*iid, &mut *object) };
        *object = ptr::null_mut();
        if *iid == IPlugFrame_iid || *iid == FUnknown_iid {
            // SAFETY: This is a live Arc-backed COM object.
            unsafe { Self::add_ref(this) };
            *object = this.cast();
            return kResultOk;
        }
        #[cfg(target_os = "linux")]
        if *iid == IRunLoop_iid
            && let Some(run_loop) = &frame.run_loop
        {
            *object = run_loop.clone().into_raw().cast();
            return kResultOk;
        }
        #[cfg(not(target_os = "linux"))]
        let _ = frame;
        kNoInterface
    }

    unsafe extern "system" fn add_ref(this: *mut FUnknown) -> u32 {
        // SAFETY: Frame instances are Arc-allocated with their interface first.
        unsafe { Arc::increment_strong_count(this.cast::<Self>()) };
        // SAFETY: The additional count owns one reference for this pointer.
        let frame = std::mem::ManuallyDrop::new(unsafe { Arc::from_raw(this.cast::<Self>()) });
        Arc::strong_count(&frame) as u32
    }

    unsafe extern "system" fn release(this: *mut FUnknown) -> u32 {
        // SAFETY: The caller transfers one owned COM reference.
        let frame = unsafe { Arc::from_raw(this.cast::<Self>()) };
        (Arc::strong_count(&frame) - 1) as u32
    }

    unsafe extern "system" fn resize_view(
        this: *mut IPlugFrame,
        view: *mut IPlugView,
        rect: *mut ViewRect,
    ) -> tresult {
        // SAFETY: COM callers supply a live frame for the duration of the call.
        let frame = unsafe { &*this.cast::<Self>() };
        if thread::current().id() != frame.ui_thread
            || view.is_null()
            || frame.view.load(Ordering::Relaxed) != view
        {
            return kInvalidArgument;
        }
        // SAFETY: VST3 supplies a live rectangle for the duration of the call.
        let Some(rect) = (unsafe { rect.as_ref() }) else {
            return kInvalidArgument;
        };
        if dimensions(rect).is_none() {
            return kInvalidArgument;
        }
        let Some(window) = frame.window.lock().ok().and_then(|window| *window) else {
            return kResultFalse;
        };
        if frame.resizing.swap(true, Ordering::Relaxed) {
            return kResultFalse;
        }
        // SAFETY: Retain the frame if the native call triggers detachment and frame release.
        unsafe { Self::add_ref(this.cast()) };
        // SAFETY: The UI thread keeps the attached native window live for this callback.
        let result = unsafe { (window.resize)(window.context as *mut c_void, rect) };
        frame.resizing.store(false, Ordering::Relaxed);
        // SAFETY: Return the reference retained above.
        unsafe { Self::release(this.cast()) };
        result
    }
}

pub(super) struct Editor {
    view: ComPtr<IPlugView>,
    frame: RefCell<Option<Arc<Frame>>>,
}

fn dimensions(rect: &ViewRect) -> Option<(i32, i32)> {
    let width = rect.right.checked_sub(rect.left)?;
    let height = rect.bottom.checked_sub(rect.top)?;
    (width > 0 && height > 0).then_some((width, height))
}

pub(super) unsafe fn create_editor(
    controller: *mut IEditController,
    platform: *const c_char,
) -> *mut Editor {
    if platform.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: Controller is borrowed from a live plug-in host.
    let Some(controller) = (unsafe { ComRef::from_raw(controller) }) else {
        return ptr::null_mut();
    };
    // SAFETY: Each factory result transfers an owned plug-in reference.
    let view = unsafe {
        ComPtr::from_raw(controller.createView(ViewType::kEditor))
            .or_else(|| ComPtr::from_raw(controller.createView(ptr::null())))
            .or_else(|| controller.cast::<IPlugView>())
    };
    let Some(view) = view else {
        return ptr::null_mut();
    };
    // SAFETY: Native shell passes a static VST3 platform name.
    if unsafe { view.isPlatformTypeSupported(platform) } != kResultTrue {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(Editor {
        view,
        frame: RefCell::new(None),
    }))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_editor_destroy(editor: *mut Editor) {
    if !editor.is_null() {
        // SAFETY: Qt transfers its uniquely owned handle after deleting the detached widget.
        drop(unsafe { Box::from_raw(editor) });
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_editor_can_resize(editor: &Editor) -> bool {
    // SAFETY: The native caller borrows a live editor on its UI thread.
    unsafe { editor.view.canResize() == kResultTrue }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_editor_size(editor: &Editor, rect: &mut ViewRect) -> bool {
    // SAFETY: The native caller supplies a writable rectangle and a live editor.
    unsafe { editor.view.getSize(rect) == kResultOk }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_editor_constrain(editor: &Editor, rect: &mut ViewRect) -> tresult {
    // SAFETY: The native caller supplies a writable rectangle and a live editor.
    unsafe { editor.view.checkSizeConstraint(rect) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_editor_on_size(editor: &Editor, rect: &mut ViewRect) -> tresult {
    // SAFETY: The native caller supplies a writable rectangle and a live editor.
    unsafe { editor.view.onSize(rect) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_editor_scale(editor: &Editor, scale: f32) {
    use vst3::Steinberg::{IPlugViewContentScaleSupport, IPlugViewContentScaleSupportTrait};
    if let Some(support) = editor.view.cast::<IPlugViewContentScaleSupport>() {
        // SAFETY: QueryInterface returned a live scale-support interface.
        unsafe { support.setContentScaleFactor(scale) };
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_editor_attach(
    editor: &Editor,
    parent: *mut c_void,
    platform: *const c_char,
    context: *mut c_void,
    resize: Resize,
    host_context: *mut FUnknown,
) -> bool {
    if parent.is_null() || platform.is_null() || editor.frame.borrow().is_some() {
        return false;
    }
    let frame = Arc::new(Frame {
        interface: IPlugFrame {
            vtbl: &Frame::VTABLE,
        },
        ui_thread: thread::current().id(),
        view: AtomicPtr::new(editor.view.as_ptr()),
        window: Mutex::new(Some(Window {
            context: context as usize,
            resize,
        })),
        resizing: AtomicBool::new(false),
        #[cfg(target_os = "linux")]
        // SAFETY: The native module lends its live application context during attachment.
        run_loop: unsafe { ComRef::from_raw(host_context) }
            .and_then(|context| context.cast::<IRunLoop>()),
    });
    #[cfg(not(target_os = "linux"))]
    let _ = host_context;
    let interface = Arc::into_raw(frame.clone()).cast_mut().cast::<IPlugFrame>();
    editor.frame.replace(Some(frame));
    // SAFETY: Qt provides the native platform parent and holds it until removal.
    let attached = unsafe {
        editor.view.setFrame(interface);
        let attached = editor.view.attached(parent, platform) == kResultOk;
        Frame::release(interface.cast());
        attached
    };
    if attached {
        return true;
    }
    let frame = editor.frame.take();
    if let Some(frame) = &frame
        && let Ok(mut window) = frame.window.lock()
    {
        *window = None;
    }
    // SAFETY: The view stays owned until native teardown completes.
    unsafe { editor.view.setFrame(ptr::null_mut()) };
    false
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_editor_detach(editor: &Editor) {
    if let Some(frame) = editor.frame.take() {
        // Invalidate the widget callback while keeping the frame available to the plug-in.
        if let Ok(mut window) = frame.window.lock() {
            *window = None;
        }
        // SAFETY: Both the frame and view remain live through native detachment.
        unsafe {
            editor.view.removed();
            editor.view.setFrame(ptr::null_mut());
        }
        frame.view.store(ptr::null_mut(), Ordering::Relaxed);
    }
}

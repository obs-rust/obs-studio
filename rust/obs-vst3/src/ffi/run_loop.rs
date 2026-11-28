use super::run_loop_registry::{Error, Registry};
use std::ffi::c_void;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};
use vst3::Steinberg::Linux::{
    IEventHandler, IEventHandlerTrait, IRunLoop, IRunLoopTrait, ITimerHandler, ITimerHandlerTrait,
};
use vst3::Steinberg::{kInvalidArgument, kResultFalse, kResultTrue, tresult};
use vst3::{Class, ComPtr, ComRef, ComWrapper};

#[derive(Clone)]
enum Handler {
    Event(Arc<ComPtr<IEventHandler>>),
    Timer(Arc<ComPtr<ITimerHandler>>),
}

impl PartialEq for Handler {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Event(a), Self::Event(b)) => a.as_ptr() == b.as_ptr(),
            (Self::Timer(a), Self::Timer(b)) => a.as_ptr() == b.as_ptr(),
            _ => false,
        }
    }
}

type WatchFd = unsafe extern "C" fn(usize, u64, i32);
type WatchTimer = unsafe extern "C" fn(usize, u64, i32);
type Unwatch = unsafe extern "C" fn(usize, u64);

struct RunLoop {
    ui_thread: ThreadId,
    registry: Mutex<Registry<Handler>>,
    context: usize,
    watch_fd: WatchFd,
    watch_timer: WatchTimer,
    unwatch: Unwatch,
}

impl Class for RunLoop {
    type Interfaces = (IRunLoop,);
}

impl RunLoop {
    fn on_ui_thread(&self) -> bool {
        thread::current().id() == self.ui_thread
    }

    fn dispatch(&self, token: u64, fd: i32) {
        if !self.on_ui_thread() {
            return;
        }
        let handler = self
            .registry
            .lock()
            .ok()
            .and_then(|registry| registry.dispatch(token));
        match handler {
            // SAFETY: The cloned Arc retains the COM reference during reentrant notification.
            Some(Handler::Event(handler)) => unsafe { handler.onFDIsSet(fd) },
            // SAFETY: The cloned Arc retains the COM reference during reentrant notification.
            Some(Handler::Timer(handler)) => unsafe { handler.onTimer() },
            None => (),
        }
    }

    fn stop(&self) {
        let previous = self.registry.lock().ok().map(|mut registry| {
            let previous = std::mem::take(&mut *registry);
            registry.stop();
            previous
        });
        // Drop outside the registry lock because a plug-in can run code during release.
        drop(previous);
    }
}

impl IRunLoopTrait for RunLoop {
    unsafe fn registerEventHandler(&self, handler: *mut IEventHandler, fd: i32) -> tresult {
        if !self.on_ui_thread() || fd < 0 {
            return kInvalidArgument;
        }
        // SAFETY: VST3 passes a live handler for the duration of the call.
        let Some(handler) = (unsafe { ComRef::from_raw(handler) }) else {
            return kInvalidArgument;
        };
        let handler = Handler::Event(Arc::new(handler.to_com_ptr()));
        let token = self
            .registry
            .lock()
            .ok()
            .and_then(|mut registry| registry.register_fd(fd, handler.clone()).ok());
        let Some(token) = token else {
            return kInvalidArgument;
        };
        // SAFETY: Successful registration implies the native shell is still live on this UI thread.
        unsafe { (self.watch_fd)(self.context, token, fd) };
        kResultTrue
    }

    unsafe fn unregisterEventHandler(&self, handler: *mut IEventHandler) -> tresult {
        if !self.on_ui_thread() {
            return kInvalidArgument;
        }
        // SAFETY: VST3 passes a live handler for the duration of the call.
        let Some(handler) = (unsafe { ComRef::from_raw(handler) }) else {
            return kInvalidArgument;
        };
        let handler = Handler::Event(Arc::new(handler.to_com_ptr()));
        let removed = self
            .registry
            .lock()
            .ok()
            .map(|mut registry| registry.unregister_fd_handler(&handler))
            .unwrap_or_default();
        if removed.is_empty() {
            return kResultFalse;
        }
        // Drop the removed COM references after releasing the lock and cancelling Qt delivery.
        for (token, _) in &removed {
            // SAFETY: The native shell owns and invalidates the source of each registered token.
            unsafe { (self.unwatch)(self.context, *token) };
        }
        kResultTrue
    }

    unsafe fn registerTimer(&self, handler: *mut ITimerHandler, milliseconds: u64) -> tresult {
        if !self.on_ui_thread() || milliseconds == 0 {
            return kInvalidArgument;
        }
        // SAFETY: VST3 passes a live handler for the duration of the call.
        let Some(handler) = (unsafe { ComRef::from_raw(handler) }) else {
            return kInvalidArgument;
        };
        let handler = Handler::Timer(Arc::new(handler.to_com_ptr()));
        let registration = self
            .registry
            .lock()
            .ok()
            .map(|mut registry| registry.register_timer(milliseconds, handler.clone()));
        match registration {
            Some(Ok(token)) => {
                // SAFETY: Successful registration implies the native shell is still live on this UI thread.
                unsafe { (self.watch_timer)(self.context, token, milliseconds as i32) };
                kResultTrue
            }
            Some(Err(Error::Invalid)) => kInvalidArgument,
            _ => kResultFalse,
        }
    }

    unsafe fn unregisterTimer(&self, handler: *mut ITimerHandler) -> tresult {
        if !self.on_ui_thread() {
            return kInvalidArgument;
        }
        // SAFETY: VST3 passes a live handler for the duration of the call.
        let Some(handler) = (unsafe { ComRef::from_raw(handler) }) else {
            return kInvalidArgument;
        };
        let handler = Handler::Timer(Arc::new(handler.to_com_ptr()));
        let removed = self
            .registry
            .lock()
            .ok()
            .and_then(|mut registry| registry.unregister_timer(&handler));
        let Some((token, _retained)) = removed else {
            return kResultFalse;
        };
        // SAFETY: The native shell owns and invalidates the source of the registered token.
        unsafe { (self.unwatch)(self.context, token) };
        kResultTrue
    }
}

type Handle = ComWrapper<RunLoop>;

#[unsafe(no_mangle)]
extern "C" fn vst3_rust_run_loop_create(
    context: usize,
    watch_fd: WatchFd,
    watch_timer: WatchTimer,
    unwatch: Unwatch,
) -> *mut c_void {
    Box::into_raw(Box::new(ComWrapper::new(RunLoop {
        ui_thread: thread::current().id(),
        registry: Mutex::new(Registry::new()),
        context,
        watch_fd,
        watch_timer,
        unwatch,
    })))
    .cast()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_run_loop_interface(handle: *const c_void) -> *mut IRunLoop {
    // SAFETY: The Qt shell borrows its live owned handle.
    unsafe { handle.cast::<Handle>().as_ref() }
        .and_then(|host| host.as_com_ref::<IRunLoop>())
        .map_or(ptr::null_mut(), |interface| interface.as_ptr())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_run_loop_dispatch(handle: *const c_void, token: u64, fd: i32) {
    // SAFETY: The Qt shell owns the handle throughout delivery.
    if let Some(host) = unsafe { handle.cast::<Handle>().as_ref() }.cloned() {
        host.dispatch(token, fd);
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_rust_run_loop_destroy(handle: *mut c_void) {
    if !handle.is_null() {
        // SAFETY: Qt stops its sources before transferring the exactly once owned handle.
        let host = unsafe { Box::from_raw(handle.cast::<Handle>()) };
        host.stop();
    }
}

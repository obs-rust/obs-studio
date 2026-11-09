//! Frontend callbacks stop the control thread before libobs shutdown.
#![allow(unsafe_code, reason = "resolve the already-loaded OBS frontend ABI")]
use obs_mcp::{
    backend::ObsBackend,
    platform::{EndpointOptions, SocketServer},
};
use std::{
    ffi::c_void,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};
type Callback = unsafe extern "C" fn(i32, *mut c_void);
type Registration = unsafe extern "C" fn(Callback, *mut c_void);
struct Worker {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}
struct State {
    worker: Option<Worker>,
    remove: Registration,
    _library: libloading::os::unix::Library,
}
static STATE: Mutex<Option<State>> = Mutex::new(None);
fn stop(state: &mut State) {
    if let Some(worker) = state.worker.take() {
        worker.stop.store(true, Ordering::Release);
        let _ = worker.thread.join();
    }
}
unsafe extern "C" fn event(event: i32, _: *mut c_void) {
    if event == 17
        && let Ok(mut state) = STATE.lock()
        && let Some(state) = state.as_mut()
    {
        stop(state);
    }
}
pub fn version() -> u32 {
    let lib = libloading::os::unix::Library::this();
    // SAFETY: the host defines the documented no-argument OBS version function.
    unsafe {
        lib.get::<unsafe extern "C" fn() -> u32>(b"obs_get_version\0")
            .map_or(0, |f| f())
    }
}
pub fn load() -> bool {
    match start() {
        Ok(()) => true,
        Err(e) => {
            eprintln!("OBS control module: {e}");
            false
        }
    }
}
fn start() -> Result<(), String> {
    let mut slot = STATE.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Ok(());
    }
    let library = libloading::os::unix::Library::this();
    // SAFETY: symbols have the signatures in obs-frontend-api.h; library stays owned by State.
    let (add, remove) = unsafe {
        (
            *library
                .get::<Registration>(b"obs_frontend_add_event_callback\0")
                .map_err(|e| e.to_string())?,
            *library
                .get::<Registration>(b"obs_frontend_remove_event_callback\0")
                .map_err(|e| e.to_string())?,
        )
    };
    let frontend = Frontend::resolve(&library)?;
    let stop_flag = Arc::new(AtomicBool::new(false));
    let thread_stop = stop_flag.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let thread = std::thread::spawn(move || {
        let server = match SocketServer::bind(&EndpointOptions::default()) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
                return;
            }
        };
        let backend =
            Rc::new(ObsBackend::attached(Rc::new(frontend)).with_cancellation(thread_stop.clone()));
        if tx.send(Ok(())).is_ok()
            && let Err(e) = server.run(&thread_stop, || ObsBackend::server(&backend))
        {
            eprintln!("OBS control socket: {e}");
        }
    });
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => (),
        other => {
            stop_flag.store(true, Ordering::Release);
            let _ = thread.join();
            return Err(format!("control socket startup: {other:?}"));
        }
    }
    // SAFETY: callback is static, userdata null and callback lifetime ends on unload.
    unsafe {
        add(event, std::ptr::null_mut());
    }
    *slot = Some(State {
        worker: Some(Worker {
            stop: stop_flag,
            thread,
        }),
        remove,
        _library: library,
    });
    Ok(())
}
pub fn unload() {
    if let Ok(mut slot) = STATE.lock()
        && let Some(mut state) = slot.take()
    {
        // SAFETY: unregister the exact static callback before dropping the library.
        unsafe {
            (state.remove)(event, std::ptr::null_mut());
        }
        stop(&mut state);
    }
}

type GetSource = unsafe extern "C" fn() -> *mut c_void;
type GetName = unsafe extern "C" fn(*mut c_void) -> *const std::ffi::c_char;
type Release = unsafe extern "C" fn(*mut c_void);
#[derive(Clone, Copy)]
struct Frontend {
    current: GetSource,
    set: unsafe extern "C" fn(*mut c_void),
    source: unsafe extern "C" fn(*const std::ffi::c_char) -> *mut c_void,
    name: GetName,
    release: Release,
    record: GetSource,
    stream: GetSource,
    output_name: GetName,
    output_release: Release,
    queue: unsafe extern "C" fn(i32, unsafe extern "C" fn(*mut c_void), *mut c_void, bool),
}
impl Frontend {
    fn resolve(lib: &libloading::os::unix::Library) -> Result<Self, String> {
        // SAFETY: documented frontend/libobs symbols, held by State for all worker calls.
        unsafe {
            Ok(Self {
                current: *lib
                    .get(b"obs_frontend_get_current_scene\0")
                    .map_err(|e| e.to_string())?,
                set: *lib
                    .get(b"obs_frontend_set_current_scene\0")
                    .map_err(|e| e.to_string())?,
                source: *lib
                    .get(b"obs_get_source_by_name\0")
                    .map_err(|e| e.to_string())?,
                name: *lib
                    .get(b"obs_source_get_name\0")
                    .map_err(|e| e.to_string())?,
                release: *lib
                    .get(b"obs_source_release\0")
                    .map_err(|e| e.to_string())?,
                record: *lib
                    .get(b"obs_frontend_get_recording_output\0")
                    .map_err(|e| e.to_string())?,
                stream: *lib
                    .get(b"obs_frontend_get_streaming_output\0")
                    .map_err(|e| e.to_string())?,
                output_name: *lib
                    .get(b"obs_output_get_name\0")
                    .map_err(|e| e.to_string())?,
                output_release: *lib
                    .get(b"obs_output_release\0")
                    .map_err(|e| e.to_string())?,
                queue: *lib.get(b"obs_queue_task\0").map_err(|e| e.to_string())?,
            })
        }
    }
    fn name_of(&self, get: GetSource, name: GetName, release: Release) -> Option<String> {
        // SAFETY: getter returns an owned OBS reference; copied name is used before release.
        unsafe {
            let source = get();
            if source.is_null() {
                return None;
            }
            let text = name(source);
            let result = (!text.is_null()).then(|| {
                std::ffi::CStr::from_ptr(text)
                    .to_string_lossy()
                    .into_owned()
            });
            release(source);
            result
        }
    }
}
struct SceneChange {
    frontend: Frontend,
    name: std::ffi::CString,
}
unsafe extern "C" fn change_scene(data: *mut c_void) {
    // SAFETY: synchronous graphics queue borrows the stack request until this returns.
    let request = unsafe { &*data.cast::<SceneChange>() };
    // SAFETY: source is retained across the frontend call, which itself retains queued UI data.
    unsafe {
        let source = (request.frontend.source)(request.name.as_ptr());
        if !source.is_null() {
            (request.frontend.set)(source);
            (request.frontend.release)(source);
        }
    }
}
impl obs_mcp::backend::AttachedFrontend for Frontend {
    fn current_scene(&self) -> Option<String> {
        self.name_of(self.current, self.name, self.release)
    }
    fn set_scene(&self, request: &obs_mcp::tools::scenes::NameInput) -> Result<(), String> {
        let mut change = SceneChange {
            frontend: *self,
            name: std::ffi::CString::new(request.name.as_str()).map_err(|e| e.to_string())?,
        };
        // SAFETY: OBS_TASK_GRAPHICS (1) forces the frontend setter's nonblocking UI path.
        // The synchronous graphics task completes before this stack request drops. Thus
        // frontend EXIT can join the worker without waiting on a blocked UI callback.
        unsafe {
            (self.queue)(1, change_scene, (&raw mut change).cast(), true);
        }
        Ok(())
    }
    fn output_name(&self, request: &obs_mcp::tools::outputs::OutputRequest) -> Option<String> {
        let get = match request.kind {
            obs_mcp::tools::outputs::OutputKind::Record => self.record,
            obs_mcp::tools::outputs::OutputKind::Stream => self.stream,
        };
        self.name_of(get, self.output_name, self.output_release)
    }
}

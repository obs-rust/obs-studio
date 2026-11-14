//! Windows: the watchdog process (#4589).
//!
//! OBS duplicates its own process handle (`SYNCHRONIZE |
//! PROCESS_TERMINATE`) and creates a manual-reset arm event, both
//! inheritable, and starts the helper with only those two in its handle
//! list (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`, as `obs_util`'s pipe does).
//! Right after `CreateProcessW` it closes the process handle and makes the
//! event non-inheritable, so the processes that OBS starts later do not
//! inherit them. The helper gets the two handle values through its
//! environment.
//!
//! The Win32 calls are declared here against kernel32.lib, as in
//! `obs_util::platform::windows::pipe`, and not through windows-sys (it
//! would bring its own import records; LNK2005 in obs.dll).

use std::ffi::{OsStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use std::time::Duration;

type Handle = *mut c_void;
type Bool = i32;

const HANDLE_FLAG_INHERIT: u32 = 0x0000_0001;
const STARTF_USESTDHANDLES: u32 = 0x0000_0100;
const STARTF_FORCEOFFFEEDBACK: u32 = 0x0000_0080;
const CREATE_UNICODE_ENVIRONMENT: u32 = 0x0000_0400;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
const SYNCHRONIZE: u32 = 0x0010_0000;
const PROCESS_TERMINATE: u32 = 0x0000_0001;
const INFINITE: u32 = 0xffff_ffff;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 0x102;

#[repr(C)]
struct SecurityAttributes {
    length: u32,
    security_descriptor: *mut c_void,
    inherit_handle: Bool,
}

#[repr(C)]
struct StartupInfoW {
    cb: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_count_chars: u32,
    y_count_chars: u32,
    fill_attribute: u32,
    flags: u32,
    show_window: u16,
    reserved2_size: u16,
    reserved2: *mut u8,
    std_input: Handle,
    std_output: Handle,
    std_error: Handle,
}

#[repr(C)]
struct StartupInfoExW {
    startup_info: StartupInfoW,
    attribute_list: *mut c_void,
}

#[repr(C)]
struct ProcessInformation {
    process: Handle,
    thread: Handle,
    process_id: u32,
    thread_id: u32,
}

// The sizes the Windows SDK gives these structs.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(
    size_of::<SecurityAttributes>() == 24
        && size_of::<StartupInfoW>() == 104
        && size_of::<StartupInfoExW>() == 112
        && size_of::<ProcessInformation>() == 24
);

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CloseHandle(object: Handle) -> Bool;
    fn GetCurrentProcess() -> Handle;
    fn DuplicateHandle(
        source_process: Handle,
        source: Handle,
        target_process: Handle,
        target: *mut Handle,
        access: u32,
        inherit: Bool,
        options: u32,
    ) -> Bool;
    fn CreateEventW(
        attributes: *const SecurityAttributes,
        manual_reset: Bool,
        initial_state: Bool,
        name: *const u16,
    ) -> Handle;
    fn SetEvent(event: Handle) -> Bool;
    fn SetHandleInformation(object: Handle, mask: u32, flags: u32) -> Bool;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
    fn WaitForMultipleObjects(
        count: u32,
        handles: *const Handle,
        wait_all: Bool,
        milliseconds: u32,
    ) -> u32;
    fn TerminateProcess(process: Handle, exit_code: u32) -> Bool;
    fn CreateProcessW(
        application_name: *const u16,
        command_line: *mut u16,
        process_attributes: *const SecurityAttributes,
        thread_attributes: *const SecurityAttributes,
        inherit_handles: Bool,
        creation_flags: u32,
        environment: *const c_void,
        current_directory: *const u16,
        startup_info: *const StartupInfoW,
        process_information: *mut ProcessInformation,
    ) -> Bool;
    fn InitializeProcThreadAttributeList(
        list: *mut c_void,
        count: u32,
        flags: u32,
        size: *mut usize,
    ) -> Bool;
    fn UpdateProcThreadAttribute(
        list: *mut c_void,
        flags: u32,
        attribute: usize,
        value: *const c_void,
        size: usize,
        previous: *mut c_void,
        return_size: *mut usize,
    ) -> Bool;
    fn DeleteProcThreadAttributeList(list: *mut c_void);
}

/// The started helper: OBS's end of the arm event, kept as its value so
/// the type is `Send`.
pub struct Helper {
    event: usize,
}

/// What the helper's wait ended with.
pub enum Wait {
    /// OBS ended by itself.
    ParentEnded,
    /// OBS was armed and still ran after the timeout.
    TimedOut,
    /// A wait failed.
    Failed,
}

fn handle(value: u64) -> Handle {
    // A handle value is pointer sized; it came from a usize.
    ptr::without_provenance_mut(usize::try_from(value).unwrap_or(0))
}

/// One argument of a command line, quoted as `CommandLineToArgvW` reads it.
fn push_quoted(line: &mut Vec<u16>, arg: &OsStr) {
    line.push(u16::from(b'"'));
    let mut backslashes = 0usize;
    for unit in arg.encode_wide() {
        if unit == u16::from(b'\\') {
            backslashes += 1;
            continue;
        }
        let doubled = if unit == u16::from(b'"') {
            backslashes * 2 + 1
        } else {
            backslashes
        };
        line.extend(std::iter::repeat_n(u16::from(b'\\'), doubled));
        backslashes = 0;
        line.push(unit);
    }
    line.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes * 2));
    line.push(u16::from(b'"'));
}

/// The environment block: this process's environment without `env`'s
/// keys, then `env`.
fn environment_block(env: &[(&str, &OsStr)]) -> Vec<u16> {
    let ours = |key: &OsStr| {
        env.iter()
            .any(|(k, _)| OsStr::new(k).eq_ignore_ascii_case(key))
    };
    let mut block = Vec::new();
    let inherited = std::env::vars_os().filter(|(k, _)| !ours(k));
    for (key, value) in inherited.chain(
        env.iter()
            .map(|(k, v)| (OsStr::new(k).to_owned(), v.to_os_string())),
    ) {
        block.extend(key.encode_wide());
        block.push(u16::from(b'='));
        block.extend(value.encode_wide());
        block.push(0);
    }
    block.push(0);
    block
}

/// Starts `program` with `args` as the helper, with `env` and the two
/// handle values added to its environment.
pub fn spawn_helper(program: &Path, args: &[&OsStr], env: &[(&str, &OsStr)]) -> Option<Helper> {
    let attributes = SecurityAttributes {
        length: u32::try_from(size_of::<SecurityAttributes>()).ok()?,
        security_descriptor: ptr::null_mut(),
        inherit_handle: 1,
    };
    let mut parent: Handle = ptr::null_mut();
    // SAFETY: the pseudo handle of this process is valid; `parent` receives
    // a new handle that is closed below.
    let duplicated = unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            GetCurrentProcess(),
            GetCurrentProcess(),
            &mut parent,
            SYNCHRONIZE | PROCESS_TERMINATE,
            1,
            0,
        )
    };
    if duplicated == 0 {
        return None;
    }
    // SAFETY: an unnamed manual-reset event, not set, inheritable for now.
    let event = unsafe { CreateEventW(&attributes, 1, 0, ptr::null()) };
    if event.is_null() {
        close(parent);
        return None;
    }

    let parent_value = parent.addr().to_string();
    let event_value = event.addr().to_string();
    let mut all_env: Vec<(&str, &OsStr)> = env.to_vec();
    all_env.push((crate::helper::ENV_PARENT, OsStr::new(&parent_value)));
    all_env.push((crate::helper::ENV_ARM, OsStr::new(&event_value)));
    let block = environment_block(&all_env);

    let mut line = Vec::new();
    push_quoted(&mut line, program.as_os_str());
    for arg in args {
        line.push(u16::from(b' '));
        push_quoted(&mut line, arg);
    }
    line.push(0);
    let mut application: Vec<u16> = program.as_os_str().encode_wide().collect();
    application.push(0);

    let started = create_process(&application, &mut line, &block, &[parent, event]);
    // The helper has its own copies now, or none: OBS's children must not
    // inherit these.
    close(parent);
    if started {
        // SAFETY: `event` is a valid handle owned by this process.
        unsafe {
            SetHandleInformation(event, HANDLE_FLAG_INHERIT, 0);
        }
        Some(Helper {
            event: event.addr(),
        })
    } else {
        close(event);
        None
    }
}

/// `CreateProcessW` with only `inherit` in the child's handle list, no
/// window and no standard handles. Closes the child's process and thread
/// handles; returns whether it started.
fn create_process(application: &[u16], line: &mut [u16], env: &[u16], inherit: &[Handle]) -> bool {
    let mut size = 0usize;
    // SAFETY: a null list with a valid size pointer only reports the size.
    unsafe {
        InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut size);
    }
    let mut storage = vec![0usize; size.div_ceil(size_of::<usize>()).max(1)];
    let list = storage.as_mut_ptr().cast::<c_void>();
    // SAFETY: `list` covers `size` bytes.
    if unsafe { InitializeProcThreadAttributeList(list, 1, 0, &mut size) } == 0 {
        return false;
    }
    let started = (|| {
        // SAFETY: `list` is initialized; `inherit` outlives CreateProcessW,
        // the last use of the list.
        let updated = unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                inherit.as_ptr().cast(),
                size_of_val(inherit),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if updated == 0 {
            return false;
        }
        // SAFETY: plain C structs for which all-zero is the documented
        // initial state.
        let mut si: StartupInfoExW = unsafe { core::mem::zeroed() };
        let mut pi: ProcessInformation = unsafe { core::mem::zeroed() };
        si.startup_info.cb = u32::try_from(size_of::<StartupInfoExW>()).unwrap_or(0);
        // No standard handles: the helper writes only to the log file.
        si.startup_info.flags = STARTF_USESTDHANDLES | STARTF_FORCEOFFFEEDBACK;
        si.attribute_list = list;
        // SAFETY: `application` and `env` are NUL-terminated, `line` is a
        // writable NUL-terminated buffer, `si` and `pi` are valid.
        let ok = unsafe {
            CreateProcessW(
                application.as_ptr(),
                line.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                env.as_ptr().cast(),
                ptr::null(),
                (&raw const si).cast(),
                &mut pi,
            )
        };
        if ok == 0 {
            return false;
        }
        close(pi.thread);
        close(pi.process);
        true
    })();
    // SAFETY: `list` was initialized above and is deleted once.
    unsafe {
        DeleteProcThreadAttributeList(list);
    }
    drop(storage);
    started
}

fn close(h: Handle) {
    // SAFETY: `h` is a handle this process owns and closes once.
    unsafe {
        CloseHandle(h);
    }
}

/// Sets the helper's arm event.
pub fn arm_helper(helper: &Helper) -> bool {
    // SAFETY: the event handle stays open for the life of the process.
    unsafe { SetEvent(ptr::without_provenance_mut(helper.event)) != 0 }
}

/// In the helper: waits until OBS ends or arms it; once armed, waits on
/// OBS for `timeout`.
pub fn helper_wait(parent: u64, arm: u64, timeout: Duration) -> Wait {
    let handles = [handle(parent), handle(arm)];
    // SAFETY: two handles this process inherited; an invalid one fails the
    // wait.
    match unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) } {
        WAIT_OBJECT_0 => return Wait::ParentEnded,
        r if r == WAIT_OBJECT_0 + 1 => {}
        _ => return Wait::Failed,
    }
    let ms = u32::try_from(timeout.as_millis()).unwrap_or(INFINITE - 1);
    // SAFETY: as above.
    match unsafe { WaitForSingleObject(handles[0], ms) } {
        WAIT_OBJECT_0 => Wait::ParentEnded,
        WAIT_TIMEOUT => Wait::TimedOut,
        _ => Wait::Failed,
    }
}

/// In the helper: ends OBS with `code`.
pub fn terminate_parent(parent: u64, code: i32) -> bool {
    // SAFETY: the inherited handle has PROCESS_TERMINATE.
    unsafe { TerminateProcess(handle(parent), code.cast_unsigned()) != 0 }
}

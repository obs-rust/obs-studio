//! Windows half of the pipe port: `libobs/util/pipe-windows.c`.
//!
//! The C is kept call for call rather than going through
//! [`std::process::Command`], whose observable behavior differs on Windows:
//! it resolves the program itself instead of letting `CreateProcessW` parse
//! the command line (so the search order differs) and it cannot set
//! `STARTF_FORCEOFFFEEDBACK`. So this module calls Win32 directly and keeps
//! its `unsafe` to the handle calls.
//!
//! Intentional difference: when creating a pipe fails half-way, C leaks the
//! handles it already made (both stderr pipe ends, or the stdio pair when
//! the second `CreatePipe` fails); this closes them. Only the process's
//! handle count can tell.

use core::ffi::{c_int, c_void};
use core::ptr;

use crate::pipe::{Args, windows_command_line};

// The Win32 calls, declared here against kernel32.lib like `ffi::bmem`
// declares `malloc`. windows-sys would bring its own import records for
// DLLs std also imports, and linking all of libobs_rust.lib into obs.dll
// then fails with duplicate `__IMPORT_DESCRIPTOR`s (LNK2005).

type Handle = *mut c_void;
type Bool = i32;

const HANDLE_FLAG_INHERIT: u32 = 0x0000_0001;
const STARTF_FORCEOFFFEEDBACK: u32 = 0x0000_0080;
const STARTF_USESTDHANDLES: u32 = 0x0000_0100;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
const INFINITE: u32 = 0xffff_ffff;
const CP_UTF8: u32 = 65001;

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
#[cfg(target_pointer_width = "32")]
const _: () = assert!(
    size_of::<SecurityAttributes>() == 12
        && size_of::<StartupInfoW>() == 68
        && size_of::<StartupInfoExW>() == 72
        && size_of::<ProcessInformation>() == 16
);

#[link(name = "kernel32")]
#[allow(non_snake_case)]
unsafe extern "system" {
    fn CloseHandle(object: Handle) -> Bool;
    fn GetLastError() -> u32;
    fn SetHandleInformation(object: Handle, mask: u32, flags: u32) -> Bool;
    fn MultiByteToWideChar(
        code_page: u32,
        flags: u32,
        multi_byte: *const u8,
        multi_byte_len: i32,
        wide: *mut u16,
        wide_len: i32,
    ) -> i32;
    fn CreatePipe(
        read: *mut Handle,
        write: *mut Handle,
        attributes: *const SecurityAttributes,
        size: u32,
    ) -> Bool;
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
        previous_value: *mut c_void,
        return_size: *mut usize,
    ) -> Bool;
    fn DeleteProcThreadAttributeList(list: *mut c_void);
    fn WaitForSingleObject(object: Handle, milliseconds: u32) -> u32;
    fn GetExitCodeProcess(process: Handle, exit_code: *mut u32) -> Bool;
    fn ReadFile(
        file: Handle,
        buffer: *mut u8,
        to_read: u32,
        read: *mut u32,
        overlapped: *mut c_void,
    ) -> Bool;
    fn WriteFile(
        file: Handle,
        buffer: *const u8,
        to_write: u32,
        written: *mut u32,
        overlapped: *mut c_void,
    ) -> Bool;
}

/// A spawned child with its stdio pipes. Mirrors `struct os_process_pipe` in
/// `pipe-windows.c`: `handle` is the child's stdout in read mode and its
/// stdin in write mode, and stderr is always captured.
pub struct Pipe {
    read_pipe: bool,
    handle: Handle,
    handle_err: Handle,
    process: Handle,
}

fn close(handle: Handle) {
    // SAFETY: every handle passed here was returned by CreatePipe or
    // CreateProcessW and is closed once.
    unsafe {
        CloseHandle(handle);
    }
}

/// `create_pipe`: an anonymous pipe whose ends are inheritable.
fn create_pipe() -> Option<(Handle, Handle)> {
    let sa = SecurityAttributes {
        length: size_of::<SecurityAttributes>() as u32,
        security_descriptor: ptr::null_mut(),
        inherit_handle: 1,
    };
    let (mut input, mut output) = (ptr::null_mut(), ptr::null_mut());
    // SAFETY: both out pointers are valid locals and `sa` outlives the call.
    if unsafe { CreatePipe(&mut input, &mut output, &sa, 0) } == 0 {
        return None;
    }
    Some((input, output))
}

/// Clears `HANDLE_FLAG_INHERIT`, so the child does not get our end.
fn keep_private(handle: Handle) -> bool {
    // SAFETY: `handle` is an open pipe end.
    unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) != 0 }
}

/// `os_utf8_to_wcs_ptr` over the Windows `utf8_to_wchar`: a UTF-8 BOM is
/// dropped, invalid input becomes U+FFFD (`MultiByteToWideChar` with no
/// flags), and a failed conversion is an empty string. NUL-terminated.
fn utf8_to_wide(text: &[u8]) -> Vec<u16> {
    let text = text.strip_prefix(&[0xef, 0xbb, 0xbf][..]).unwrap_or(text);
    let mut wide = Vec::new();
    let len = i32::try_from(text.len()).unwrap_or(0);
    if len > 0 {
        // SAFETY: `text` covers `len` bytes; a null output asks for the size.
        let size =
            unsafe { MultiByteToWideChar(CP_UTF8, 0, text.as_ptr(), len, ptr::null_mut(), 0) };
        if size > 0 {
            wide.resize(size as usize, 0);
            // SAFETY: `wide` holds `size` units.
            let done = unsafe {
                MultiByteToWideChar(CP_UTF8, 0, text.as_ptr(), len, wide.as_mut_ptr(), size)
            };
            wide.truncate(done.max(0) as usize);
        }
    }
    wide.push(0);
    wide
}

/// `create_process`: runs `cmd_line` with the given stdio handles (null
/// where not redirected), without a console window or the busy cursor. The
/// child inherits those stdio handles and nothing else
/// (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`): with every inheritable handle it
/// kept e.g. an SRT Media Source's socket open after the source closed it
/// (obsproject/obs-studio#4596). On failure `log` gets `GetLastError()`.
fn create_process(
    cmd_line: &[u8],
    stdin: Handle,
    stdout: Handle,
    stderr: Handle,
    log: impl FnOnce(u32),
) -> Option<Handle> {
    let mut cmd_line = utf8_to_wide(cmd_line);
    let inherit: Vec<Handle> = [stdin, stdout, stderr]
        .into_iter()
        .filter(|h| !h.is_null())
        .collect();

    // The attribute list is opaque; the first call asks for its size. It
    // holds pointers, so back it with pointer-aligned storage.
    let mut size = 0usize;
    // SAFETY: a null list with a valid size pointer only reports the size.
    unsafe {
        InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut size);
    }
    let mut storage = vec![0usize; size.div_ceil(size_of::<usize>()).max(1)];
    let list = storage.as_mut_ptr().cast::<c_void>();
    // SAFETY: `list` covers `size` bytes.
    if unsafe { InitializeProcThreadAttributeList(list, 1, 0, &mut size) } == 0 {
        // SAFETY: reads the calling thread's last error.
        log(unsafe { GetLastError() });
        return None;
    }
    let result = (|| {
        // SAFETY: `list` is initialized; `inherit` outlives CreateProcessW,
        // which is the last use of the list.
        if !inherit.is_empty()
            && unsafe {
                UpdateProcThreadAttribute(
                    list,
                    0,
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                    inherit.as_ptr().cast(),
                    inherit.len() * size_of::<Handle>(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            } == 0
        {
            return Err(());
        }

        // SAFETY: StartupInfoExW and ProcessInformation are plain C structs
        // for which all-zero is the documented initial state.
        let mut si: StartupInfoExW = unsafe { core::mem::zeroed() };
        let mut pi: ProcessInformation = unsafe { core::mem::zeroed() };
        si.startup_info.cb = size_of::<StartupInfoExW>() as u32;
        si.startup_info.flags = STARTF_USESTDHANDLES | STARTF_FORCEOFFFEEDBACK;
        si.startup_info.std_input = stdin;
        si.startup_info.std_output = stdout;
        si.startup_info.std_error = stderr;
        si.attribute_list = list;

        // SAFETY: `cmd_line` is a writable NUL-terminated buffer, as
        // CreateProcessW requires, and `si`/`pi` are valid for the call.
        let ok = unsafe {
            CreateProcessW(
                ptr::null(),
                cmd_line.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                Bool::from(!inherit.is_empty()),
                CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT,
                ptr::null(),
                ptr::null(),
                (&si as *const StartupInfoExW).cast(),
                &mut pi,
            )
        };
        if ok == 0 {
            return Err(());
        }
        close(pi.thread);
        Ok(pi.process)
    })();
    // Read the error before DeleteProcThreadAttributeList can change it.
    // Not logging the command line is intentional (C): it may hold stream
    // keys.
    let process = match result {
        Ok(process) => Some(process),
        Err(()) => {
            // SAFETY: reads the calling thread's last error.
            log(unsafe { GetLastError() });
            None
        }
    };
    // SAFETY: `list` was initialized above and is deleted once.
    unsafe {
        DeleteProcThreadAttributeList(list);
    }
    drop(storage);
    process
}

impl Pipe {
    /// Mirrors `os_process_pipe_create`, without logging spawn failures.
    pub fn create(cmd_line: Option<&[u8]>, type_: &[u8]) -> Option<Self> {
        Self::create_logged(cmd_line, type_, |_| {})
    }

    /// Mirrors `os_process_pipe_create`: `type_` must start with `r`
    /// (read the child's stdout) or `w` (write its stdin). `log` gets the
    /// error code when `CreateProcessW` fails, which C logs.
    pub fn create_logged(
        cmd_line: Option<&[u8]>,
        type_: &[u8],
        log: impl FnOnce(u32),
    ) -> Option<Self> {
        let cmd_line = cmd_line?;
        let read_pipe = match type_.first() {
            Some(b'r') => true,
            Some(b'w') => false,
            _ => return None,
        };
        let (input, output) = create_pipe()?;
        let Some((err_input, err_output)) = create_pipe() else {
            close(output);
            close(input);
            return None;
        };
        let close_all = || {
            close(output);
            close(input);
            close(err_output);
            close(err_input);
        };

        // Our ends: stdout's read end or stdin's write end, and stderr's
        // read end.
        let (ours, theirs) = if read_pipe {
            (input, output)
        } else {
            (output, input)
        };
        if !keep_private(ours) || !keep_private(err_input) {
            close_all();
            return None;
        }

        let (stdin, stdout) = if read_pipe {
            (ptr::null_mut(), output)
        } else {
            (input, ptr::null_mut())
        };
        let Some(process) = create_process(cmd_line, stdin, stdout, err_output, log) else {
            close_all();
            return None;
        };

        close(theirs);
        close(err_output);
        Some(Self {
            read_pipe,
            handle: ours,
            handle_err: err_input,
            process,
        })
    }

    /// Mirrors `os_process_pipe_create2`, without logging spawn failures.
    pub fn create2(args: Option<&Args>, type_: &[u8]) -> Option<Self> {
        Self::create2_logged(args, type_, |_| {})
    }

    /// Mirrors `os_process_pipe_create2`: Windows has no argv API, so the
    /// arguments are quoted into one command line
    /// ([`windows_command_line`]). A null `args` is `None`; C would
    /// dereference it.
    pub fn create2_logged(
        args: Option<&Args>,
        type_: &[u8],
        log: impl FnOnce(u32),
    ) -> Option<Self> {
        let cmd_line = windows_command_line(args?);
        Self::create_logged(Some(&cmd_line), type_, log)
    }

    /// Mirrors `os_process_pipe_destroy`: closes our ends, waits for the
    /// child and returns its exit code as `(int)DWORD`, or 0 if it cannot
    /// be read.
    pub fn destroy(self) -> c_int {
        close(self.handle);
        close(self.handle_err);
        let mut code = 0u32;
        // SAFETY: `process` is the child's open process handle and `code`
        // a valid local.
        let ret = unsafe {
            WaitForSingleObject(self.process, INFINITE);
            if GetExitCodeProcess(self.process, &mut code) != 0 {
                code as c_int
            } else {
                0
            }
        };
        close(self.process);
        ret
    }

    /// One `ReadFile` on `handle`; 0 on error or end of file. The length is
    /// cast to `DWORD` as in C.
    fn read_from(handle: Handle, buf: &mut [u8]) -> usize {
        let mut read = 0u32;
        // SAFETY: `buf` covers the length passed (truncated like C's cast)
        // and `read` is a valid local.
        let ok = unsafe {
            ReadFile(
                handle,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut read,
                ptr::null_mut(),
            )
        };
        if ok != 0 { read as usize } else { 0 }
    }

    /// Mirrors `os_process_pipe_read`: one read of the child's stdout; 0 for
    /// a write-mode pipe.
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        if !self.read_pipe {
            return 0;
        }
        Self::read_from(self.handle, buf)
    }

    /// Mirrors `os_process_pipe_read_err`: one read of the child's stderr,
    /// in either mode.
    pub fn read_err(&mut self, buf: &mut [u8]) -> usize {
        if self.handle_err.is_null() {
            return 0;
        }
        Self::read_from(self.handle_err, buf)
    }

    /// Mirrors `os_process_pipe_write`: one `WriteFile` to the child's
    /// stdin (a short write is returned, not retried); 0 for a read-mode
    /// pipe or on error.
    pub fn write(&mut self, buf: &[u8]) -> usize {
        if self.read_pipe {
            return 0;
        }
        let mut written = 0u32;
        // SAFETY: `buf` covers the length passed (truncated like C's cast)
        // and `written` is a valid local.
        let ok = unsafe {
            WriteFile(
                self.handle,
                buf.as_ptr(),
                buf.len() as u32,
                &mut written,
                ptr::null_mut(),
            )
        };
        if ok != 0 { written as usize } else { 0 }
    }
}

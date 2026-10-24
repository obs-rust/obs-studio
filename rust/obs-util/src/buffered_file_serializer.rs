//! Intentional C difference: Large buffered recording chunks complete when the configured queue limit is smaller than a chunk.
//! Intentional C difference: Buffered recording producers wake and report failure after a disk write error.
//!
//! Safe core for `libobs/util/buffered-file-serializer.c`.
//!
//! A background thread drains a byte queue and writes the file. Chunks under
//! 64 KiB stay off disk until free, until a seek makes the offset
//! discontinuous, or until the next piece would overflow `chunk_size`.
//! `SERIALIZE_SEEK_END` subtracts from that virtual write position. It does
//! not use the file size.

use std::fs::File;
use std::io::{ErrorKind, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

/// C `DEFAULT_BUF_SIZE`. Used when `max_bufsize` is 0.
const DEFAULT_BUF_SIZE: usize = 256 * 1048576;
/// C `DEFAULT_CHUNK_SIZE`. Used when `chunk_size` is 0.
const DEFAULT_CHUNK_SIZE: usize = 1048576;
/// `deque_reserve` at init. Backpressure uses `max(capacity, buffer_size)`.
const INITIAL_CAPACITY: usize = 1048576;
/// Hardcoded in the C writer. Smaller pending chunks wait for more data.
const SMALL_WRITE: usize = 65536;
const HEADER_LEN: usize = 16;

/// `enum serialize_seek_type` in `libobs/util/serializer.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekType {
    Start,
    Current,
    End,
}

/// Map a C `serialize_seek_type`. Anything else leaves the position alone,
/// matching the C `switch` with no default.
pub fn seek_type_from_c(code: i32) -> Option<SeekType> {
    match code {
        0 => Some(SeekType::Start),
        1 => Some(SeekType::Current),
        2 => Some(SeekType::End),
        _ => None,
    }
}

/// Auto-reset event. `os_event_signal` sticks until `os_event_wait` consumes
/// it. `os_event_reset` clears a signal that has not been waited on.
struct AutoEvent {
    flag: Mutex<bool>,
    cv: Condvar,
    #[cfg(test)]
    waiters: std::sync::atomic::AtomicUsize,
}

impl AutoEvent {
    fn new() -> Self {
        Self {
            flag: Mutex::new(false),
            cv: Condvar::new(),
            #[cfg(test)]
            waiters: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn wait(&self, error: Option<&AtomicBool>) {
        let mut flag = self.flag.lock().unwrap();
        while !*flag && !error.is_some_and(|error| error.load(Ordering::SeqCst)) {
            #[cfg(test)]
            self.waiters.fetch_add(1, Ordering::SeqCst);
            flag = self.cv.wait(flag).unwrap();
            #[cfg(test)]
            self.waiters.fetch_sub(1, Ordering::SeqCst);
        }
        *flag = false;
    }

    fn signal(&self) {
        let mut flag = self.flag.lock().unwrap();
        *flag = true;
        self.cv.notify_one();
    }

    fn signal_all(&self) {
        let mut flag = self.flag.lock().unwrap();
        *flag = true;
        self.cv.notify_all();
    }

    fn reset(&self) {
        *self.flag.lock().unwrap() = false;
    }
}

/// Byte queue with the C `deque` growth rule: start at 1 MiB, double when
/// `size` passes `capacity`. The wait check uses this logical capacity, not
/// the allocation `VecDeque` happens to have.
struct ByteBuf {
    bytes: std::collections::VecDeque<u8>,
    capacity: usize,
}

impl ByteBuf {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: std::collections::VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn push(&mut self, data: &[u8]) {
        self.bytes.extend(data);
        if self.bytes.len() <= self.capacity {
            return;
        }
        let mut new_cap = self.capacity.saturating_mul(2);
        if self.bytes.len() > new_cap {
            new_cap = self.bytes.len();
        }
        self.capacity = new_cap;
    }

    fn push_header(&mut self, seek_offset: u64, data_length: u64) {
        let mut raw = [0u8; HEADER_LEN];
        raw[..8].copy_from_slice(&seek_offset.to_ne_bytes());
        raw[8..].copy_from_slice(&data_length.to_ne_bytes());
        self.push(&raw);
    }

    fn peek_header(&self) -> (u64, u64) {
        let mut raw = [0u8; HEADER_LEN];
        copy_prefix(&self.bytes, &mut raw);
        let seek = u64::from_ne_bytes(raw[..8].try_into().unwrap());
        let len = u64::from_ne_bytes(raw[8..].try_into().unwrap());
        (seek, len)
    }

    fn pop_front(&mut self, n: usize) {
        let _ = self.bytes.drain(..n);
    }

    fn pop_into(&mut self, dst: &mut [u8]) {
        assert!(self.bytes.len() >= dst.len());
        copy_prefix(&self.bytes, dst);
        self.pop_front(dst.len());
    }
}

fn copy_prefix(bytes: &std::collections::VecDeque<u8>, dst: &mut [u8]) {
    let n = dst.len();
    let (front, back) = bytes.as_slices();
    if n <= front.len() {
        dst.copy_from_slice(&front[..n]);
        return;
    }
    let first = front.len();
    dst[..first].copy_from_slice(front);
    dst[first..].copy_from_slice(&back[..n - first]);
}

struct Shared {
    buf: ByteBuf,
    next_pos: u64,
    buffer_size: usize,
    chunk_size: usize,
}

struct Inner {
    output_error: AtomicBool,
    shutdown_requested: AtomicBool,
    space: AutoEvent,
    new_data: AutoEvent,
    shared: Mutex<Shared>,
}

/// Buffered file output. Drop joins the writer thread and closes the file.
pub struct Output {
    inner: Arc<Inner>,
    join: Option<JoinHandle<()>>,
}

impl Output {
    /// `buffered_file_serializer_init`. A zero size selects the C default.
    ///
    /// Returns `None` when the file cannot be created or the writer thread
    /// cannot start. `s` is left untouched in that case, as in C (the open
    /// fails before the serializer is stored).
    pub fn create(path: &Path, max_bufsize: usize, chunk_size: usize) -> Option<Self> {
        let file = File::create(path).ok()?;
        let buffer_size = if max_bufsize == 0 {
            DEFAULT_BUF_SIZE
        } else {
            max_bufsize
        };
        let chunk_size = if chunk_size == 0 {
            DEFAULT_CHUNK_SIZE
        } else {
            chunk_size
        };
        let inner = Arc::new(Inner {
            output_error: AtomicBool::new(false),
            shutdown_requested: AtomicBool::new(false),
            space: AutoEvent::new(),
            new_data: AutoEvent::new(),
            shared: Mutex::new(Shared {
                buf: ByteBuf::with_capacity(INITIAL_CAPACITY),
                next_pos: 0,
                buffer_size,
                chunk_size,
            }),
        });
        let thread_inner = Arc::clone(&inner);
        let join = std::thread::Builder::new()
            .name("buffered writer i/o thread".to_string())
            .spawn(move || io_thread(thread_inner, file))
            .ok()?;
        Some(Self {
            inner,
            join: Some(join),
        })
    }

    /// `buffered_file_serializer_init_defaults`.
    pub fn create_defaults(path: &Path) -> Option<Self> {
        Self::create(path, 0, 0)
    }

    /// `file_output_write`. A zero-length buffer returns 0 and queues nothing.
    ///
    /// Once `output_error` is set, further calls return 0 even if an earlier
    /// piece of this call was already queued. That is the C loop.
    pub fn write(&self, buf: &[u8]) -> usize {
        if buf.is_empty() {
            return 0;
        }
        let mut offset = 0;
        while offset < buf.len() {
            let mut shared = self.inner.shared.lock().unwrap();
            if self.inner.output_error.load(Ordering::SeqCst) {
                return 0;
            }
            let chunk_size = shared.chunk_size;
            let next_chunk = (buf.len() - offset).min(chunk_size);
            // Even an empty queue must fit one chunk and its header.
            let cap = shared
                .buf
                .capacity
                .max(shared.buffer_size)
                .max(next_chunk.saturating_add(HEADER_LEN));
            let free = cap.saturating_sub(shared.buf.len());
            if free < next_chunk + HEADER_LEN {
                // Reset while holding the data lock, then wait. The writer
                // thread signals `space` under that same lock.
                self.inner.space.reset();
                drop(shared);
                self.inner.space.wait(Some(&self.inner.output_error));
                continue;
            }
            let mut num_chunks = free / (next_chunk + HEADER_LEN);
            while offset < buf.len() && num_chunks > 0 {
                num_chunks -= 1;
                let n = (buf.len() - offset).min(chunk_size);
                let pos = shared.next_pos;
                shared.buf.push_header(pos, n as u64);
                shared.buf.push(&buf[offset..offset + n]);
                shared.next_pos = pos.wrapping_add(n as u64);
                offset += n;
            }
            self.inner.new_data.signal();
        }
        buf.len()
    }

    /// `file_output_seek`. Offsets wrap the way a `uint64_t` does in C.
    /// `None` is an unknown seek code: the position does not change.
    pub fn seek(&self, offset: i64, kind: Option<SeekType>) -> i64 {
        if self.inner.output_error.load(Ordering::SeqCst) {
            return -1;
        }
        let mut shared = self.inner.shared.lock().unwrap();
        if let Some(kind) = kind {
            match kind {
                SeekType::Start => shared.next_pos = offset as u64,
                SeekType::Current => {
                    shared.next_pos = shared.next_pos.wrapping_add(offset as u64);
                }
                SeekType::End => {
                    shared.next_pos = shared.next_pos.wrapping_sub(offset as u64);
                }
            }
        }
        shared.next_pos as i64
    }

    /// `file_output_get_pos`.
    pub fn pos(&self) -> i64 {
        if self.inner.output_error.load(Ordering::SeqCst) {
            return -1;
        }
        self.inner.shared.lock().unwrap().next_pos as i64
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        self.inner.shutdown_requested.store(true, Ordering::SeqCst);
        if let Ok(guard) = self.inner.shared.lock() {
            self.inner.new_data.signal();
            drop(guard);
        }
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

/// `os_fseeki64(..., SEEK_SET)` takes an `int64_t`. A wrapped position is
/// negative and the seek fails, leaving the file offset where it was.
fn seek_set<W: Seek>(file: &mut W, pos: u64) {
    let off = pos as i64;
    if off < 0 {
        return;
    }
    let _ = file.seek(SeekFrom::Start(off as u64));
}

fn write_full<W: Write>(file: &mut W, mut buf: &[u8]) -> bool {
    while !buf.is_empty() {
        match file.write(buf) {
            Ok(0) => return false,
            Ok(n) => buf = &buf[n..],
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return false,
        }
    }
    true
}

fn io_thread<W: Write + Seek>(inner: Arc<Inner>, mut file: W) {
    let chunk_size = inner.shared.lock().unwrap().chunk_size;
    let mut chunk = vec![0u8; chunk_size];
    let mut chunk_used = 0usize;
    let mut want_seek = false;
    let mut force_flush = false;
    let mut current_seek = 0u64;
    let mut next_seek = 0u64;
    // Assigned at the start of every chunk pass, before either read.
    let mut shutting_down;

    'outer: loop {
        inner.new_data.wait(None);
        'chunks: loop {
            {
                let mut shared = inner.shared.lock().unwrap();
                shutting_down = inner.shutdown_requested.load(Ordering::SeqCst);
                loop {
                    if shared.buf.len() < HEADER_LEN {
                        break;
                    }
                    let (seek_offset, data_length) = shared.buf.peek_header();
                    let data_len = data_length as usize;
                    if seek_offset != current_seek {
                        if chunk_used > 0 || want_seek {
                            force_flush = true;
                            break;
                        }
                        want_seek = true;
                        next_seek = seek_offset;
                        current_seek = seek_offset;
                    }
                    if data_len.saturating_add(chunk_used) > chunk_size {
                        force_flush = true;
                        break;
                    }
                    shared.buf.pop_front(HEADER_LEN);
                    shared
                        .buf
                        .pop_into(&mut chunk[chunk_used..chunk_used + data_len]);
                    chunk_used += data_len;
                    current_seek = current_seek.wrapping_add(data_length);
                }
                inner.space.signal();
                if !force_flush && (chunk_used == 0 || (chunk_used < SMALL_WRITE && !shutting_down))
                {
                    inner.new_data.reset();
                    break 'chunks;
                }
            }
            if want_seek {
                seek_set(&mut file, next_seek);
                current_seek = next_seek.wrapping_add(chunk_used as u64);
                want_seek = false;
                if chunk_used == 0 {
                    force_flush = false;
                    continue;
                }
            }
            if !write_full(&mut file, &chunk[..chunk_used]) {
                // Pair error publication and the wake with the producer's
                // check/reset under the queue lock to prevent lost wakeups.
                let _shared = inner.shared.lock().unwrap();
                inner.output_error.store(true, Ordering::SeqCst);
                inner.space.signal_all();
                return;
            }
            chunk_used = 0;
            force_flush = false;
        }
        if shutting_down {
            break 'outer;
        }
    }
}

#[cfg(test)]
mod failure_tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    struct FailingWriter(mpsc::SyncSender<()>, mpsc::Receiver<()>);

    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            self.0.send(()).unwrap();
            self.1.recv().unwrap();
            Err(std::io::Error::other("disk full"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl Seek for FailingWriter {
        fn seek(&mut self, _: SeekFrom) -> std::io::Result<u64> {
            Ok(0)
        }
    }

    fn error_wakes_producers(producers: usize) {
        let inner = Arc::new(Inner {
            output_error: AtomicBool::new(false),
            shutdown_requested: AtomicBool::new(false),
            space: AutoEvent::new(),
            new_data: AutoEvent::new(),
            shared: Mutex::new(Shared {
                buf: ByteBuf::with_capacity(INITIAL_CAPACITY),
                next_pos: 0,
                buffer_size: INITIAL_CAPACITY,
                chunk_size: SMALL_WRITE,
            }),
        });
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = mpsc::sync_channel(1);
        let worker_inner = Arc::clone(&inner);
        let join = std::thread::spawn(move || {
            io_thread(worker_inner, FailingWriter(entered_tx, resume_rx))
        });
        let out = Arc::new(Output {
            inner: Arc::clone(&inner),
            join: Some(join),
        });
        assert_eq!(out.write(&vec![1; SMALL_WRITE]), SMALL_WRITE);
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let mut handles = Vec::new();
        for _ in 0..producers {
            let out = Arc::clone(&out);
            let done_tx = done_tx.clone();
            handles.push(std::thread::spawn(move || {
                let result = out.write(&vec![2; 2 * INITIAL_CAPACITY]);
                done_tx.send(result).unwrap();
            }));
        }
        drop(done_tx);
        let deadline = Instant::now() + Duration::from_secs(5);
        // The test-only waiter count proves every producer entered its
        // condition-variable wait before the writer publishes the disk error.
        while inner.space.waiters.load(Ordering::SeqCst) < producers {
            assert!(
                Instant::now() < deadline,
                "producers did not reach backpressure"
            );
            std::thread::yield_now();
        }
        resume_tx.send(()).unwrap();
        for _ in 0..producers {
            assert_eq!(
                done_rx
                    .recv_timeout(Duration::from_secs(5))
                    .expect("producer stuck after write error"),
                0
            );
        }
        for handle in handles {
            handle.join().unwrap();
        }
        drop(out);
    }

    #[test]
    fn write_error_wakes_backpressured_producer() {
        error_wakes_producers(1);
    }

    #[test]
    fn write_error_wakes_every_backpressured_producer() {
        error_wakes_producers(4);
    }
}

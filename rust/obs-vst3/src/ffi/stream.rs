use crate::state::{SeekOrigin, StateStream};
use std::ffi::c_void;
use std::slice;
use std::sync::Mutex;
use vst3::Steinberg::{
    IBStream, IBStreamTrait, kInternalError, kInvalidArgument, kResultOk, tresult,
};
use vst3::{Class, ComPtr, ComWrapper};

pub(super) struct Stream(Mutex<StateStream>);

impl Stream {
    pub(super) fn create(bytes: Vec<u8>) -> (ComWrapper<Self>, ComPtr<IBStream>) {
        let host = ComWrapper::new(Self(Mutex::new(StateStream::from_bytes(bytes))));
        let interface = host.to_com_ptr().expect("Stream implements IBStream");
        (host, interface)
    }

    pub(super) fn save(&self) -> Vec<u8> {
        self.0
            .lock()
            .map_or_else(|_| Vec::new(), |stream| stream.saved_bytes().to_vec())
    }
}

impl Class for Stream {
    type Interfaces = (IBStream,);
}

impl IBStreamTrait for Stream {
    unsafe fn read(&self, buffer: *mut c_void, bytes: i32, read: *mut i32) -> tresult {
        let Ok(size) = usize::try_from(bytes) else {
            return kInvalidArgument;
        };
        if buffer.is_null() && size != 0 {
            return kInvalidArgument;
        }
        let Ok(mut stream) = self.0.lock() else {
            return kInternalError;
        };
        let output = if size == 0 {
            &mut []
        } else {
            // SAFETY: IBStream's caller supplies a writable buffer of `bytes`.
            unsafe { slice::from_raw_parts_mut(buffer.cast::<u8>(), size) }
        };
        let count = stream.read(output);
        if !read.is_null() {
            // SAFETY: The optional counter points to writable caller storage.
            unsafe { read.write(count as i32) };
        }
        kResultOk
    }

    unsafe fn write(&self, buffer: *mut c_void, bytes: i32, written: *mut i32) -> tresult {
        let Ok(size) = usize::try_from(bytes) else {
            return kInvalidArgument;
        };
        if buffer.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: The IBStream caller supplies a readable span of `bytes`.
        let input = unsafe { slice::from_raw_parts(buffer.cast::<u8>(), size) };
        let Ok(mut stream) = self.0.lock() else {
            return kInternalError;
        };
        let Ok(count) = stream.write(input) else {
            return kInternalError;
        };
        if !written.is_null() {
            // SAFETY: The optional counter points to writable caller storage.
            unsafe { written.write(count as i32) };
        }
        kResultOk
    }

    unsafe fn seek(&self, position: i64, mode: i32, result: *mut i64) -> tresult {
        let Ok(mut stream) = self.0.lock() else {
            return kInternalError;
        };
        let next = match mode {
            0 => stream.seek(position, SeekOrigin::Start),
            1 => stream.seek(position, SeekOrigin::Current),
            2 => stream.seek(position, SeekOrigin::End),
            _ => Ok(stream.position()),
        };
        let Ok(next) = next else {
            return kInvalidArgument;
        };
        if !result.is_null() {
            // SAFETY: The optional result points to writable caller storage.
            unsafe { result.write(next) };
        }
        kResultOk
    }

    unsafe fn tell(&self, position: *mut i64) -> tresult {
        if position.is_null() {
            return kInvalidArgument;
        }
        let Ok(stream) = self.0.lock() else {
            return kInternalError;
        };
        // SAFETY: The required output pointer is writable caller storage.
        unsafe { position.write(stream.position()) };
        kResultOk
    }
}

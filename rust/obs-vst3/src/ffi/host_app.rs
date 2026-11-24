use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::Mutex;
use std::{mem, ptr, slice};
use vst3::Steinberg::Vst::{
    IAttributeList, IAttributeListTrait, IAudioProcessor, IComponent, IConnectionPoint,
    IEditController, IHostApplication, IHostApplicationTrait, IMessage, IMessageTrait,
    IPlugInterfaceSupport, IPlugInterfaceSupportTrait, String128, TChar,
};
use vst3::Steinberg::{
    FUnknown, TUID, kInternalError, kInvalidArgument, kResultFalse, kResultOk, tresult,
};
use vst3::{Class, ComPtr, ComWrapper, Interface};

pub(super) struct HostApp;

impl HostApp {
    pub(super) fn create() -> ComPtr<FUnknown> {
        let host = ComWrapper::new(Self);
        host.to_com_ptr::<IHostApplication>()
            .expect("HostApp implements IHostApplication")
            .upcast()
    }
}

impl Class for HostApp {
    type Interfaces = (IHostApplication, IPlugInterfaceSupport);
}

impl IHostApplicationTrait for HostApp {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        if name.is_null() {
            return kInvalidArgument;
        }
        let mut output = [0; 128];
        for (dst, character) in output.iter_mut().zip("OBS VST3 Host".encode_utf16()) {
            *dst = character;
        }
        // SAFETY: The required output points to a caller-owned SDK String128.
        unsafe { name.write(output) };
        kResultOk
    }

    unsafe fn createInstance(
        &self,
        cid: *mut TUID,
        iid: *mut TUID,
        object: *mut *mut c_void,
    ) -> tresult {
        if cid.is_null() || iid.is_null() || object.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: Both pointers address inline SDK IDs for this call.
        let (cid, iid) = unsafe { (id_bytes(&*cid), id_bytes(&*iid)) };
        let instance = if cid == IMessage::IID && iid == IMessage::IID {
            let instance = ComWrapper::new(Message::new());
            instance
                .to_com_ptr::<IMessage>()
                .expect("Message implements IMessage")
                .into_raw()
                .cast()
        } else if cid == IAttributeList::IID && iid == IAttributeList::IID {
            Attributes::create().into_raw().cast()
        } else {
            ptr::null_mut()
        };
        // SAFETY: The out pointer is writable and the caller receives an
        // owned reference to the requested object on success.
        unsafe { object.write(instance) };
        if instance.is_null() {
            kResultFalse
        } else {
            kResultOk
        }
    }
}

impl IPlugInterfaceSupportTrait for HostApp {
    unsafe fn isPlugInterfaceSupported(&self, iid: *const TUID) -> tresult {
        if iid.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: The non-null argument is a live SDK interface ID.
        let iid = unsafe { id_bytes(&*iid) };
        if [
            IComponent::IID,
            IAudioProcessor::IID,
            IEditController::IID,
            IConnectionPoint::IID,
        ]
        .contains(&iid)
        {
            kResultOk
        } else {
            kResultFalse
        }
    }
}

fn id_bytes(id: &TUID) -> [u8; 16] {
    id.map(|byte| byte.to_ne_bytes()[0])
}

struct Message {
    id: Mutex<Option<CString>>,
    attributes: ComPtr<IAttributeList>,
}

impl Message {
    fn new() -> Self {
        Self {
            id: Mutex::new(None),
            attributes: Attributes::create(),
        }
    }
}

impl Class for Message {
    type Interfaces = (IMessage,);
}

impl IMessageTrait for Message {
    unsafe fn getMessageID(&self) -> *const c_char {
        self.id.lock().map_or(ptr::null(), |id| {
            id.as_ref().map_or(ptr::null(), |id| id.as_ptr())
        })
    }

    unsafe fn setMessageID(&self, id: *const c_char) {
        if let Ok(mut target) = self.id.lock() {
            // SAFETY: Non-null IDs are NUL-terminated strings borrowed for
            // this call. The message owns its copy until another set.
            *target = (!id.is_null()).then(|| unsafe { CStr::from_ptr(id) }.to_owned());
        }
    }

    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attributes.as_ptr()
    }
}

enum Attribute {
    Integer(i64),
    Float(f64),
    Text(Vec<TChar>),
    Binary(Vec<u8>),
}

#[derive(Default)]
struct Attributes(Mutex<HashMap<Vec<u8>, Attribute>>);

impl Class for Attributes {
    type Interfaces = (IAttributeList,);
}

impl Attributes {
    fn create() -> ComPtr<IAttributeList> {
        ComWrapper::new(Self::default())
            .to_com_ptr()
            .expect("Attributes implements IAttributeList")
    }

    fn set(&self, id: &CStr, value: Attribute) -> tresult {
        let Ok(mut attrs) = self.0.lock() else {
            return kInternalError;
        };
        attrs.insert(id.to_bytes().to_vec(), value);
        kResultOk
    }

    fn get(&self, id: &CStr, read: impl FnOnce(Option<&Attribute>) -> tresult) -> tresult {
        let Ok(attrs) = self.0.lock() else {
            return kInternalError;
        };
        read(attrs.get(id.to_bytes()))
    }
}

impl IAttributeListTrait for Attributes {
    unsafe fn setInt(&self, id: *const c_char, value: i64) -> tresult {
        // SAFETY: The SDK caller provides a NUL-terminated identifier.
        let Some(id) = (unsafe { attr_id(id) }) else {
            return kInvalidArgument;
        };
        self.set(id, Attribute::Integer(value))
    }

    unsafe fn getInt(&self, id: *const c_char, value: *mut i64) -> tresult {
        // SAFETY: The SDK caller provides a NUL-terminated identifier.
        let Some(id) = (unsafe { attr_id(id) }) else {
            return kInvalidArgument;
        };
        self.get(id, |attr| {
            let Some(Attribute::Integer(number)) = attr else {
                return kResultFalse;
            };
            if value.is_null() {
                return kInvalidArgument;
            }
            // SAFETY: This output addresses caller-owned storage.
            unsafe { value.write(*number) };
            kResultOk
        })
    }

    unsafe fn setFloat(&self, id: *const c_char, value: f64) -> tresult {
        // SAFETY: The SDK caller provides a NUL-terminated identifier.
        let Some(id) = (unsafe { attr_id(id) }) else {
            return kInvalidArgument;
        };
        self.set(id, Attribute::Float(value))
    }

    unsafe fn getFloat(&self, id: *const c_char, value: *mut f64) -> tresult {
        // SAFETY: The SDK caller provides a NUL-terminated identifier.
        let Some(id) = (unsafe { attr_id(id) }) else {
            return kInvalidArgument;
        };
        self.get(id, |attr| {
            let Some(Attribute::Float(number)) = attr else {
                return kResultFalse;
            };
            if value.is_null() {
                return kInvalidArgument;
            }
            // SAFETY: This output addresses caller-owned storage.
            unsafe { value.write(*number) };
            kResultOk
        })
    }

    unsafe fn setString(&self, id: *const c_char, string: *const TChar) -> tresult {
        // SAFETY: The SDK caller provides a NUL-terminated identifier.
        let Some(id) = (unsafe { attr_id(id) }) else {
            return kInvalidArgument;
        };
        if string.is_null() {
            return kInvalidArgument;
        }
        let mut text = Vec::new();
        loop {
            // SAFETY: The SDK string consists of live UTF-16 units through its
            // terminating zero. Only units prior to that terminator are copied.
            let unit = unsafe { string.add(text.len()).read() };
            text.push(unit);
            if unit == 0 {
                break;
            }
        }
        self.set(id, Attribute::Text(text))
    }

    unsafe fn getString(&self, id: *const c_char, string: *mut TChar, bytes: u32) -> tresult {
        // SAFETY: The SDK caller provides a NUL-terminated identifier.
        let Some(id) = (unsafe { attr_id(id) }) else {
            return kInvalidArgument;
        };
        self.get(id, |attr| {
            let Some(Attribute::Text(text)) = attr else {
                return kResultFalse;
            };
            let count = (bytes as usize).min(mem::size_of_val(text.as_slice()));
            if count == 0 {
                return kResultOk;
            }
            if string.is_null() {
                return kInvalidArgument;
            }
            // SAFETY: This is the caller's declared writable byte span.
            unsafe { ptr::copy_nonoverlapping(text.as_ptr().cast::<u8>(), string.cast(), count) };
            kResultOk
        })
    }

    unsafe fn setBinary(&self, id: *const c_char, data: *const c_void, bytes: u32) -> tresult {
        // SAFETY: The SDK caller provides a NUL-terminated identifier.
        let Some(id) = (unsafe { attr_id(id) }) else {
            return kInvalidArgument;
        };
        let data = if bytes == 0 {
            Vec::new()
        } else {
            if data.is_null() {
                return kInvalidArgument;
            }
            // SAFETY: The caller provides the declared readable byte span.
            unsafe { slice::from_raw_parts(data.cast::<u8>(), bytes as usize) }.to_vec()
        };
        self.set(id, Attribute::Binary(data))
    }

    unsafe fn getBinary(
        &self,
        id: *const c_char,
        data: *mut *const c_void,
        bytes: *mut u32,
    ) -> tresult {
        // SAFETY: The SDK caller provides a NUL-terminated identifier.
        let Some(id) = (unsafe { attr_id(id) }) else {
            return kInvalidArgument;
        };
        if data.is_null() || bytes.is_null() {
            return kInvalidArgument;
        }
        self.get(id, |attr| {
            let Some(Attribute::Binary(blob)) = attr else {
                // SAFETY: The out length is writable even on a missing attribute.
                unsafe { bytes.write(0) };
                return kResultFalse;
            };
            // SAFETY: The two out pointers are writable caller storage. The
            // returned bytes are borrowed until this attribute is updated.
            unsafe {
                data.write(blob.as_ptr().cast());
                bytes.write(blob.len() as u32);
            }
            kResultOk
        })
    }
}

unsafe fn attr_id<'a>(id: *const c_char) -> Option<&'a CStr> {
    // SAFETY: The caller supplies the live NUL-terminated ID, if present.
    (!id.is_null()).then(|| unsafe { CStr::from_ptr(id) })
}

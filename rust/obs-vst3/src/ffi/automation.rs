use crate::parameters::{Change, Point, Queue, Transfer};
use std::ptr;
use std::sync::Mutex;
use vst3::Steinberg::Vst::{
    IParamValueQueue, IParamValueQueueTrait, IParameterChanges, IParameterChangesTrait,
};
use vst3::Steinberg::{kInternalError, kInvalidArgument, kResultOk, tresult};
use vst3::{Class, ComPtr, ComWrapper};

struct ParameterQueue(Mutex<(u32, Queue)>);

impl Class for ParameterQueue {
    type Interfaces = (IParamValueQueue,);
}

impl IParamValueQueueTrait for ParameterQueue {
    unsafe fn getParameterId(&self) -> u32 {
        self.0.lock().map_or(0, |queue| queue.0)
    }

    unsafe fn getPointCount(&self) -> i32 {
        self.0
            .lock()
            .map_or(0, |queue| queue.1.points().len() as i32)
    }

    unsafe fn getPoint(&self, index: i32, offset: *mut i32, value: *mut f64) -> tresult {
        let Ok(index) = usize::try_from(index) else {
            return kInvalidArgument;
        };
        if offset.is_null() || value.is_null() {
            return kInvalidArgument;
        }
        let Ok(queue) = self.0.lock() else {
            return kInternalError;
        };
        let Some(point) = queue.1.points().get(index) else {
            return kInvalidArgument;
        };
        // SAFETY: The two non-null out pointers designate caller-owned storage.
        unsafe {
            offset.write(point.offset);
            value.write(point.value);
        }
        kResultOk
    }

    unsafe fn addPoint(&self, offset: i32, value: f64, index: *mut i32) -> tresult {
        if index.is_null() {
            return kInvalidArgument;
        }
        let Ok(mut queue) = self.0.lock() else {
            return kInternalError;
        };
        let position = queue.1.insert(Point { offset, value });
        // SAFETY: The required out pointer designates caller-owned storage.
        unsafe { index.write(position as i32) };
        kResultOk
    }
}

struct Queues {
    entries: Vec<(ComWrapper<ParameterQueue>, ComPtr<IParamValueQueue>)>,
    used: usize,
}

impl Queues {
    fn add(
        &mut self,
        id: u32,
    ) -> Option<(usize, &ComWrapper<ParameterQueue>, *mut IParamValueQueue)> {
        let index = self.entries[..self.used]
            .iter()
            .position(|(queue, _)| queue.0.lock().is_ok_and(|queue| queue.0 == id))
            .unwrap_or(self.used);
        if index == self.used {
            if index == self.entries.len() {
                let queue = ComWrapper::new(ParameterQueue(Mutex::new((id, Queue::default()))));
                let interface = queue.to_com_ptr()?;
                self.entries.push((queue, interface));
            } else {
                let mut queue = self.entries[index].0.0.lock().ok()?;
                queue.0 = id;
                queue.1.clear();
            }
            self.used += 1;
        }
        let (queue, interface) = &self.entries[index];
        Some((index, queue, interface.as_ptr()))
    }
}

pub(super) struct Automation(Mutex<Queues>);

impl Class for Automation {
    type Interfaces = (IParameterChanges,);
}

impl Automation {
    pub(super) fn create() -> (ComWrapper<Self>, ComPtr<IParameterChanges>) {
        let host = ComWrapper::new(Self(Mutex::new(Queues {
            entries: Vec::new(),
            used: 0,
        })));
        let interface = host
            .to_com_ptr()
            .expect("Automation implements IParameterChanges");
        (host, interface)
    }

    pub(super) fn clear(&self) {
        if let Ok(mut queues) = self.0.lock() {
            queues.used = 0;
        }
    }

    pub(super) fn transfer_from(&self, source: &Transfer) {
        let Ok(mut queues) = self.0.lock() else {
            return;
        };
        while let Some(change) = source.pop() {
            if let Some((_, queue, _)) = queues.add(change.id)
                && let Ok(mut queue) = queue.0.lock()
            {
                queue.1.insert(change.point);
            }
        }
    }

    pub(super) fn transfer_to(&self, destination: &Transfer) -> bool {
        let Ok(queues) = self.0.lock() else {
            return false;
        };
        for (queue, _) in &queues.entries[..queues.used] {
            if let Ok(queue) = queue.0.lock() {
                for point in queue.1.points() {
                    destination.push(Change {
                        id: queue.0,
                        point: *point,
                    });
                }
            }
        }
        queues.used != 0
    }
}

impl IParameterChangesTrait for Automation {
    unsafe fn getParameterCount(&self) -> i32 {
        self.0.lock().map_or(0, |queues| queues.used as i32)
    }

    unsafe fn getParameterData(&self, index: i32) -> *mut IParamValueQueue {
        let Ok(index) = usize::try_from(index) else {
            return ptr::null_mut();
        };
        let Ok(queues) = self.0.lock() else {
            return ptr::null_mut();
        };
        queues.entries[..queues.used]
            .get(index)
            .map_or(ptr::null_mut(), |entry| entry.1.as_ptr())
    }

    unsafe fn addParameterData(&self, id: *const u32, index: *mut i32) -> *mut IParamValueQueue {
        if id.is_null() || index.is_null() {
            return ptr::null_mut();
        }
        let Ok(mut queues) = self.0.lock() else {
            return ptr::null_mut();
        };
        // SAFETY: The SDK caller passes a readable parameter ID.
        let Some((position, _, interface)) = queues.add(unsafe { id.read() }) else {
            return ptr::null_mut();
        };
        // SAFETY: The SDK caller reserves writable index storage.
        unsafe { index.write(position as i32) };
        interface
    }
}

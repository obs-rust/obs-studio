use crate::parameters::{Change, Point, Restart, Transfer};
use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use vst3::Steinberg::Vst::{
    IAudioProcessor, IAudioProcessorTrait, IComponent, IComponentHandler, IComponentHandlerTrait,
    IComponentTrait, IEditController, IEditControllerTrait, IUnitHandler, IUnitHandlerTrait,
    ParameterInfo,
};
use vst3::Steinberg::{kInvalidArgument, kNotImplemented, kResultOk, tresult};
use vst3::{Class, ComPtr, ComWrapper};

pub(super) struct Controls {
    pub(super) component: ComPtr<IComponent>,
    pub(super) controller: ComPtr<IEditController>,
    pub(super) processor: ComPtr<IAudioProcessor>,
    pub(super) process_lock: Mutex<()>,
    pub(super) pending_latency_restart: AtomicBool,
}

impl Controls {
    pub(super) fn restart_latency(&self) {
        let Ok(_process) = self.process_lock.try_lock() else {
            self.pending_latency_restart.store(true, Ordering::Release);
            return;
        };
        self.pending_latency_restart.store(false, Ordering::Release);
        // SAFETY: The SDK invokes this callback on the controller thread.
        // process_lock excludes the live processor's real-time call.
        unsafe {
            self.processor.setProcessing(0);
            self.component.setActive(0);
            self.component.setActive(1);
            self.processor.setProcessing(1);
            self.processor.getLatencySamples();
        }
    }
}

pub(super) struct Handler {
    controls: Weak<Controls>,
    changes: Arc<Transfer>,
}

impl Class for Handler {
    type Interfaces = (IComponentHandler, IUnitHandler);
}

impl Handler {
    pub(super) fn create(
        controls: &Arc<Controls>,
        changes: Arc<Transfer>,
    ) -> ComPtr<IComponentHandler> {
        let host = ComWrapper::new(Self {
            controls: Arc::downgrade(controls),
            changes,
        });
        host.to_com_ptr()
            .expect("Handler implements IComponentHandler")
    }
}

impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, _id: u32) -> tresult {
        kResultOk
    }

    unsafe fn performEdit(&self, id: u32, value: f64) -> tresult {
        self.changes.push(Change {
            id,
            point: Point { offset: 0, value },
        });
        kResultOk
    }

    unsafe fn endEdit(&self, _id: u32) -> tresult {
        kResultOk
    }

    unsafe fn restartComponent(&self, flags: i32) -> tresult {
        let Some(controls) = self.controls.upgrade() else {
            return kInvalidArgument;
        };
        let Some(restart) = Restart::decode(flags) else {
            return kNotImplemented;
        };
        if restart.latency_changed {
            controls.restart_latency();
        }
        if restart.values_changed {
            // SAFETY: The initialized controller is live and SDK callbacks
            // propagate changed parameter values on the controller thread.
            unsafe {
                for index in 0..controls.controller.getParameterCount() {
                    let mut info: ParameterInfo = mem::zeroed();
                    if controls.controller.getParameterInfo(index, &mut info) == kResultOk {
                        self.changes.push(Change {
                            id: info.id,
                            point: Point {
                                offset: 0,
                                value: controls.controller.getParamNormalized(info.id),
                            },
                        });
                    }
                }
            }
        }
        kResultOk
    }
}

impl IUnitHandlerTrait for Handler {
    unsafe fn notifyUnitSelection(&self, _id: i32) -> tresult {
        kResultOk
    }

    unsafe fn notifyProgramListChange(&self, _list: i32, _index: i32) -> tresult {
        kResultOk
    }
}

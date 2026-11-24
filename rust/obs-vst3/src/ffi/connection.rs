use crate::Error;
use std::thread::{self, ThreadId};
use vst3::Steinberg::Vst::{
    IComponent, IConnectionPoint, IConnectionPointTrait, IEditController, IMessage,
};
use vst3::Steinberg::{kResultFalse, kResultOk, tresult};
use vst3::{Class, ComPtr, ComWrapper};

pub(super) struct Endpoint {
    connection: ComPtr<IConnectionPoint>,
    proxy: ComPtr<IConnectionPoint>,
}

impl Endpoint {
    fn connect(
        connection: ComPtr<IConnectionPoint>,
        target: ComPtr<IConnectionPoint>,
    ) -> Result<Self, Error> {
        let bridge = ComWrapper::new(ConnectionProxy {
            target,
            owner: thread::current().id(),
        });
        let proxy = bridge
            .to_com_ptr()
            .expect("Proxy implements IConnectionPoint");
        // SAFETY: Both retained endpoints and the proxy's destination remain
        // live until this endpoint performs its matching disconnection.
        if unsafe { connection.connect(proxy.as_ptr()) } != kResultOk {
            return Err(Error("VST3 connection request failed"));
        }
        Ok(Self { connection, proxy })
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        // SAFETY: The component was connected successfully and is still
        // initialized. The pointer matches the original connection.
        unsafe { self.connection.disconnect(self.proxy.as_ptr()) };
    }
}

pub(super) fn pair(
    component: &ComPtr<IComponent>,
    controller: &ComPtr<IEditController>,
) -> Result<[Endpoint; 2], Error> {
    let processor = component
        .cast::<IConnectionPoint>()
        .ok_or(Error("VST3 component has no connection point"))?;
    let controller = controller
        .cast::<IConnectionPoint>()
        .ok_or(Error("VST3 controller has no connection point"))?;
    let to_controller = Endpoint::connect(processor.clone(), controller.clone())?;
    let to_component = Endpoint::connect(controller, processor)?;
    Ok([to_controller, to_component])
}

struct ConnectionProxy {
    target: ComPtr<IConnectionPoint>,
    owner: ThreadId,
}

impl Class for ConnectionProxy {
    type Interfaces = (IConnectionPoint,);
}

impl IConnectionPointTrait for ConnectionProxy {
    unsafe fn connect(&self, _other: *mut IConnectionPoint) -> tresult {
        kResultFalse
    }

    unsafe fn disconnect(&self, _other: *mut IConnectionPoint) -> tresult {
        kResultFalse
    }

    unsafe fn notify(&self, message: *mut IMessage) -> tresult {
        if self.owner != thread::current().id() {
            return kResultFalse;
        }
        // SAFETY: The plug-in supplies a message valid for this synchronous
        // connection call. The proxy owns its destination.
        unsafe { self.target.notify(message) }
    }
}

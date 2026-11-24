use super::automation::Automation;
use super::connection::{self, Endpoint};
use super::factory::{class_name, decode_class_id};
use super::handler::{Controls, Handler};
use super::host_app::HostApp;
use super::stream::Stream;
use crate::Error;
use crate::bus::{BusPlan, Description, Kind};
use crate::parameters::Transfer;
use crate::platform::{Module, ModuleUse};
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{mem, path::Path, ptr};
use vst3::Steinberg::Vst::{
    AudioBusBuffers, AudioBusBuffers__type0, BusInfo, IAudioProcessor, IAudioProcessorTrait,
    IComponent, IComponentHandler, IComponentTrait, IEditController, IEditControllerTrait,
    IParameterChanges, ProcessContext,
    ProcessContext_::StatesAndFlags_::{kPlaying, kRecording, kSystemTimeValid},
    ProcessData, ProcessSetup,
};
use vst3::Steinberg::{
    FUnknown, IPluginBase, IPluginBaseTrait, IPluginFactory, IPluginFactory3, IPluginFactory3Trait,
    IPluginFactoryTrait, TUID, kResultOk,
};
use vst3::{ComPtr, ComWrapper, Interface};

const AUDIO: i32 = 0;
const INPUT: i32 = 0;
const OUTPUT: i32 = 1;
const MAIN: i32 = 0;
const REALTIME: i32 = 0;
const SAMPLE_32: i32 = 0;

pub struct Engine {
    handler: Option<RegisteredHandler>,
    controls: Arc<Controls>,
    processor: ComPtr<IAudioProcessor>,
    connections: Option<[Endpoint; 2]>,
    component_init: Option<Initialized>,
    controller_init: Option<Initialized>,
    controller: ComPtr<IEditController>,
    component: ComPtr<IComponent>,
    _module: Module,
    _host: ComPtr<FUnknown>,
    input: BusBuffers,
    output: BusBuffers,
    context: ProcessContext,
    automation: AutomationState,
    pub(super) name: CString,
    pub(super) main_input: usize,
    pub(super) main_output: usize,
    pub(super) sidechain: Option<usize>,
    pub(super) max_frames: usize,
    active: bool,
    processing: bool,
}

struct AutomationState {
    from_gui: Arc<Transfer>,
    to_gui: Transfer,
    input: (ComWrapper<Automation>, ComPtr<IParameterChanges>),
    output: (ComWrapper<Automation>, ComPtr<IParameterChanges>),
    gui_pending: bool,
}

impl AutomationState {
    fn new(parameter_count: usize) -> Self {
        Self {
            from_gui: Arc::new(Transfer::new(parameter_count)),
            to_gui: Transfer::new(parameter_count),
            input: Automation::create(),
            output: Automation::create(),
            gui_pending: false,
        }
    }

    fn prepare(&self) {
        self.input.0.clear();
        self.output.0.clear();
        self.input.0.transfer_from(&self.from_gui);
    }
}

struct Initialized(ComPtr<IPluginBase>);

impl Initialized {
    fn new(base: ComPtr<IPluginBase>, host: &ComPtr<FUnknown>) -> Result<Self, Error> {
        // SAFETY: This component is a newly created, owned SDK interface.
        // Its host context outlives every initialized component/controller.
        if unsafe { base.initialize(host.as_ptr()) } == kResultOk {
            Ok(Self(base))
        } else {
            Err(Error("VST3 initialization failed"))
        }
    }
}

impl Drop for Initialized {
    fn drop(&mut self) {
        // SAFETY: This interface was initialized successfully and the owning
        // object and module remain live until the lifecycle guard is dropped.
        unsafe { self.0.terminate() };
    }
}

struct RegisteredHandler {
    controller: ComPtr<IEditController>,
    _handler: ComPtr<IComponentHandler>,
}

impl RegisteredHandler {
    fn new(controller: ComPtr<IEditController>, handler: ComPtr<IComponentHandler>) -> Self {
        // SAFETY: The controller is initialized and both interfaces remain live
        // until the guard detaches the handler, including on setup failure.
        unsafe { controller.setComponentHandler(handler.as_ptr()) };
        Self {
            controller,
            _handler: handler,
        }
    }
}

impl Drop for RegisteredHandler {
    fn drop(&mut self) {
        // SAFETY: The controller still owns a live SDK object and the handler
        // remains live for the callback to release any references it retained.
        unsafe { self.controller.setComponentHandler(ptr::null_mut()) };
    }
}

impl Engine {
    pub fn open(
        path: &Path,
        class_id: &str,
        sample_rate: f64,
        max_frames: i32,
        arrangement: u64,
    ) -> Result<Self, Error> {
        Self::with_host(
            path,
            class_id,
            sample_rate,
            max_frames,
            arrangement,
            HostApp::create(),
        )
    }

    pub(super) fn with_host(
        path: &Path,
        class_id: &str,
        sample_rate: f64,
        max_frames: i32,
        arrangement: u64,
        host: ComPtr<FUnknown>,
    ) -> Result<Self, Error> {
        if !sample_rate.is_finite() || sample_rate <= 0.0 || max_frames <= 0 {
            return Err(Error("invalid VST3 processing setup"));
        }
        let id = decode_class_id(class_id).ok_or(Error("invalid VST3 class ID"))?;
        let module = Module::open(path, ModuleUse::Host)?;
        if let Some(factory) = module.factory().cast::<IPluginFactory3>() {
            // SAFETY: The VST3 module and its components drop before this host context.
            unsafe { factory.setHostContext(host.as_ptr()) };
        }
        let name =
            class_name(module.factory(), &id).ok_or(Error("VST3 audio module class not found"))?;
        let component: ComPtr<IComponent> =
            create(module.factory(), &id).ok_or(Error("VST3 component not found"))?;
        let component_init = Initialized::new(component.clone().upcast(), &host)?;
        let (controller, controller_init) =
            if let Some(controller) = component.cast::<IEditController>() {
                (controller, None)
            } else {
                let mut controller_id = [0; 16];
                // SAFETY: The live component writes a TUID to caller storage.
                if unsafe { component.getControllerClassId(&mut controller_id) } != kResultOk {
                    return Err(Error("VST3 component did not specify a controller"));
                }
                let controller: ComPtr<IEditController> = create(module.factory(), &controller_id)
                    .ok_or(Error("VST3 controller not found"))?;
                let controller_init = Initialized::new(controller.clone().upcast(), &host)?;
                (controller, Some(controller_init))
            };
        let connections = if controller_init.is_some() {
            Some(connection::pair(&component, &controller)?)
        } else {
            None
        };
        let processor = component
            .cast::<IAudioProcessor>()
            .or_else(|| controller.cast::<IAudioProcessor>())
            .ok_or(Error("VST3 processor not found"))?;

        let controls = Arc::new(Controls {
            component: component.clone(),
            controller: controller.clone(),
            processor: processor.clone(),
            process_lock: Mutex::new(()),
            pending_latency_restart: AtomicBool::new(false),
        });
        // SAFETY: The controller owns its parameter metadata after initialization.
        let parameter_count =
            usize::try_from(unsafe { controller.getParameterCount() }).unwrap_or_default();
        let automation = AutomationState::new(parameter_count);
        let handler = RegisteredHandler::new(
            controller.clone(),
            Handler::create(&controls, Arc::clone(&automation.from_gui)),
        );
        let NegotiatedBuses {
            plan,
            input: actual_input,
            output: actual_output,
        } = negotiate_buses(&component, &processor, arrangement)?;
        let mut setup = ProcessSetup {
            processMode: REALTIME,
            symbolicSampleSize: SAMPLE_32,
            maxSamplesPerBlock: max_frames,
            sampleRate: sample_rate,
        };
        // SAFETY: The processor is live and the SDK setup object is initialized.
        if unsafe { processor.setupProcessing(&mut setup) } != kResultOk {
            return Err(Error("VST3 rejected processing setup"));
        }
        let max_frames = max_frames as usize;
        let input = BusBuffers::new(&actual_input, max_frames);
        let output = BusBuffers::new(&actual_output, max_frames);
        // SAFETY: ProcessContext contains only numeric placeholders and can
        // begin zeroed. The flags below identify the fields valid for plug-ins.
        let mut context: ProcessContext = unsafe { mem::zeroed() };
        context.sampleRate = sample_rate;
        context.state =
            u32::from_ne_bytes((kPlaying | kRecording | kSystemTimeValid).to_ne_bytes());
        let mut engine = Self {
            handler: Some(handler),
            controls,
            processor,
            connections,
            controller_init,
            controller,
            component_init: Some(component_init),
            component,
            _module: module,
            _host: host,
            input,
            output,
            context,
            automation,
            name,
            main_input: plan.main_input,
            main_output: plan.main_output,
            sidechain: plan.sidechain,
            max_frames,
            active: false,
            processing: false,
        };
        engine.activate(true)?;
        Ok(engine)
    }

    pub(super) fn sidechain_channels(&self) -> usize {
        self.sidechain.map_or(0, |index| {
            self.input
                .buses
                .get(index)
                .map_or(0, |bus| bus.channels.len().min(2))
        })
    }

    pub(super) fn controller_ptr(&self) -> *mut IEditController {
        self.controller.as_ptr()
    }

    pub fn input_channel(&mut self, bus: usize, channel: usize) -> Option<&mut [f32]> {
        Some(self.input.buses.get_mut(bus)?.channels.get_mut(channel)?)
    }

    pub fn output_channel(&self, bus: usize, channel: usize) -> Option<&[f32]> {
        Some(self.output.buses.get(bus)?.channels.get(channel)?)
    }

    pub fn activate(&mut self, active: bool) -> Result<(), Error> {
        if self.active == active {
            return Ok(());
        }
        // SAFETY: The initialized component is live and process is exclusive.
        if unsafe { self.component.setActive(u8::from(active)) } != kResultOk {
            if active {
                // SAFETY: Restore the live component after a rejected activation.
                unsafe { self.component.setActive(0) };
            }
            return Err(Error("VST3 component activation failed"));
        }
        self.active = active;
        Ok(())
    }

    pub fn set_processing(&mut self, active: bool) -> Result<(), Error> {
        if self.processing != active {
            // SAFETY: Only the exclusive owner changes the processor state.
            // Some processors do not implement this optional notification.
            unsafe { self.processor.setProcessing(u8::from(active)) };
            self.processing = active;
        }
        Ok(())
    }

    pub fn process(&mut self, frames: usize, system_time: i64) -> Result<(), Error> {
        if !self.active || !self.processing {
            return Err(Error("VST3 processor is not running"));
        }
        let _process = self
            .controls
            .process_lock
            .try_lock()
            .map_err(|_| Error("VST3 processor is restarting"))?;
        let frames = frames.min(self.max_frames) as i32;
        self.context.systemTime = system_time;
        self.context.projectTimeSamples = self
            .context
            .projectTimeSamples
            .saturating_add(i64::from(frames));
        self.input.refresh_pointers();
        self.output.refresh_pointers();
        self.automation.prepare();
        let mut data = ProcessData {
            processMode: REALTIME,
            symbolicSampleSize: SAMPLE_32,
            numSamples: frames,
            numInputs: self.input.raw.len() as i32,
            numOutputs: self.output.raw.len() as i32,
            inputs: self.input.raw.as_mut_ptr(),
            outputs: self.output.raw.as_mut_ptr(),
            inputParameterChanges: self.automation.input.1.as_ptr(),
            outputParameterChanges: self.automation.output.1.as_ptr(),
            inputEvents: ptr::null_mut(),
            outputEvents: ptr::null_mut(),
            processContext: &mut self.context,
        };
        // SAFETY: The context and planar buffers remain owned, stable and
        // uniquely accessible during this synchronous processor invocation.
        if unsafe { self.processor.process(&mut data) } == kResultOk {
            self.automation.gui_pending |= self
                .automation
                .output
                .0
                .transfer_to(&self.automation.to_gui);
            Ok(())
        } else {
            Err(Error("VST3 processor failed"))
        }
    }

    pub fn needs_gui_drain(&self) -> bool {
        self.automation.gui_pending
            || self
                .controls
                .pending_latency_restart
                .load(Ordering::Acquire)
    }

    pub fn drain_gui_changes(&mut self) {
        if self
            .controls
            .pending_latency_restart
            .load(Ordering::Acquire)
        {
            self.controls.restart_latency();
        }
        self.automation.gui_pending = false;
        while let Some(change) = self.automation.to_gui.pop() {
            // SAFETY: The GUI thread owns controller-side parameter dispatch.
            unsafe {
                self.controller
                    .setParamNormalized(change.id, change.point.value)
            };
        }
    }

    pub fn restore_state(&mut self, component: &[u8], controller: &[u8]) -> Result<(), Error> {
        if component.is_empty() {
            return Err(Error("empty VST3 component state"));
        }
        let (_, stream) = Stream::create(component.to_vec());
        // SAFETY: The component receives a live IBStream with owned bytes.
        if unsafe { self.component.setState(stream.as_ptr()) } != kResultOk {
            return Err(Error("VST3 component rejected state"));
        }
        let (_, stream) = Stream::create(component.to_vec());
        // SAFETY: The component state stream starts at zero for the controller.
        unsafe { self.controller.setComponentState(stream.as_ptr()) };
        if !controller.is_empty() {
            let (_, stream) = Stream::create(controller.to_vec());
            // SAFETY: This separate stream belongs to the initialized controller.
            unsafe { self.controller.setState(stream.as_ptr()) };
        }
        Ok(())
    }

    pub fn save_state(&self) -> Result<(Vec<u8>, Vec<u8>), Error> {
        let (component, stream) = Stream::create(Vec::new());
        // SAFETY: The component synchronously writes to the live stream.
        if unsafe { self.component.getState(stream.as_ptr()) } != kResultOk {
            return Err(Error("VST3 component failed to save state"));
        }
        let component = component.save();
        if component.is_empty() {
            return Err(Error("empty VST3 component state"));
        }
        let (controller, stream) = Stream::create(Vec::new());
        // SAFETY: This optional state stream remains live through the SDK call.
        let controller = if unsafe { self.controller.getState(stream.as_ptr()) } == kResultOk {
            controller.save()
        } else {
            Vec::new()
        };
        Ok((component, controller))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.set_processing(false);
        let _ = self.activate(false);
        let _ = self.handler.take();
        let _ = self.connections.take();
        // Follow PlugProvider's original order after both endpoints disconnect.
        let _ = self.component_init.take();
        let _ = self.controller_init.take();
    }
}

fn create<I: Interface>(factory: &ComPtr<IPluginFactory>, cid: &TUID) -> Option<ComPtr<I>> {
    let mut object = ptr::null_mut();
    // SAFETY: The factory borrows exact SDK interface/class IDs and returns one
    // owned reference to the requested interface on success.
    unsafe {
        (factory.createInstance(cid.as_ptr(), I::IID.as_ptr().cast(), &mut object) == kResultOk)
            .then(|| ComPtr::from_raw(object.cast::<I>()))
            .flatten()
    }
}

struct NegotiatedBuses {
    plan: BusPlan,
    input: Vec<Description>,
    output: Vec<Description>,
}

fn negotiate_buses(
    component: &ComPtr<IComponent>,
    processor: &ComPtr<IAudioProcessor>,
    arrangement: u64,
) -> Result<NegotiatedBuses, Error> {
    let input_description = buses(component, processor, INPUT)?;
    let output_description = buses(component, processor, OUTPUT)?;
    let plan = BusPlan::select(&input_description, &output_description, arrangement)
        .map_err(|_| Error("unsupported VST3 bus layout"))?;
    for (direction, choices) in [(INPUT, &plan.input), (OUTPUT, &plan.output)] {
        for (index, bus) in choices.iter().enumerate() {
            // SAFETY: The component exposed this index during bus discovery.
            unsafe { component.activateBus(AUDIO, direction, index as i32, u8::from(bus.enabled)) };
        }
    }
    let mut inputs: Vec<_> = plan.input.iter().map(|bus| bus.arrangement).collect();
    let mut outputs: Vec<_> = plan.output.iter().map(|bus| bus.arrangement).collect();
    // SAFETY: Both lists own writable storage for the declared count.
    let accepted = unsafe {
        processor.setBusArrangements(
            inputs.as_mut_ptr(),
            inputs.len() as i32,
            outputs.as_mut_ptr(),
            outputs.len() as i32,
        )
    } == kResultOk;
    let actual_input = buses(component, processor, INPUT)?;
    let actual_output = buses(component, processor, OUTPUT)?;
    if !accepted && !plan.matches_active_arrangements(&actual_input, &actual_output) {
        return Err(Error("VST3 processor rejected requested speaker layout"));
    }
    Ok(NegotiatedBuses {
        plan,
        input: actual_input,
        output: actual_output,
    })
}

fn buses(
    component: &ComPtr<IComponent>,
    processor: &ComPtr<IAudioProcessor>,
    direction: i32,
) -> Result<Vec<Description>, Error> {
    // SAFETY: Both queried interfaces are live and direction is an SDK enum.
    let count = unsafe { component.getBusCount(AUDIO, direction) };
    (0..count)
        .map(|index| {
            // SAFETY: BusInfo is a plain SDK record of numbers and one inline string.
            let mut info: BusInfo = unsafe { mem::zeroed() };
            // SAFETY: The index is within the discovered count and info is writable.
            if unsafe { component.getBusInfo(AUDIO, direction, index, &mut info) } != kResultOk {
                return Err(Error("VST3 failed to describe audio bus"));
            }
            let channels = usize::try_from(info.channelCount)
                .map_err(|_| Error("invalid VST3 audio channel count"))?;
            let mut arrangement = 0;
            // SAFETY: Arrangement result is valid writable caller-owned storage.
            let arrangement =
                (unsafe { processor.getBusArrangement(direction, index, &mut arrangement) }
                    == kResultOk)
                    .then_some(arrangement);
            let kind = if info.busType == MAIN {
                Kind::Main
            } else {
                Kind::Auxiliary
            };
            Ok(Description {
                kind,
                channels,
                arrangement,
            })
        })
        .collect()
}

struct BusBuffers {
    raw: Vec<AudioBusBuffers>,
    buses: Vec<BusBuffer>,
}

struct BusBuffer {
    pointers: Vec<*mut f32>,
    channels: Vec<Box<[f32]>>,
}

impl BusBuffers {
    fn new(buses: &[Description], frames: usize) -> Self {
        let mut buffers = Self {
            raw: Vec::with_capacity(buses.len()),
            buses: Vec::with_capacity(buses.len()),
        };
        for bus in buses {
            let channels: Vec<_> = (0..bus.channels)
                .map(|_| vec![0.0; frames].into_boxed_slice())
                .collect();
            let pointers = vec![ptr::null_mut(); bus.channels];
            buffers.raw.push(AudioBusBuffers {
                numChannels: bus.channels as i32,
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 {
                    channelBuffers32: ptr::null_mut(),
                },
            });
            buffers.buses.push(BusBuffer { pointers, channels });
        }
        buffers
    }

    fn refresh_pointers(&mut self) {
        for (raw, bus) in self.raw.iter_mut().zip(&mut self.buses) {
            for (pointer, channel) in bus.pointers.iter_mut().zip(&mut bus.channels) {
                *pointer = channel.as_mut_ptr();
            }
            raw.__field0.channelBuffers32 = bus.pointers.as_mut_ptr();
        }
    }
}

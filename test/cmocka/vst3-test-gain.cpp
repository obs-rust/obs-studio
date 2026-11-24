// SPDX-License-Identifier: MIT
// Deterministic VST3 effect used by the OBS host integration test.

#include "pluginterfaces/base/ibstream.h"
#include "pluginterfaces/base/ipluginbase.h"
#include "pluginterfaces/gui/iplugview.h"
#include "pluginterfaces/vst/ivstaudioprocessor.h"
#include "pluginterfaces/vst/ivsteditcontroller.h"
#include "pluginterfaces/vst/vstspeaker.h"

#include <array>
#include <atomic>
#include <cmath>
#include <cstring>
#include <new>

using namespace Steinberg;
using namespace Steinberg::Vst;

namespace {
constexpr TUID gain_id = INLINE_UID(0x57B71384, 0xC2EE4713, 0xB586F2FD, 0x7FDB08A1);
constexpr TUID second_gain_id = INLINE_UID(0x6BD4700D, 0x163F4219, 0xA265882F, 0x52B373E8);

bool same_id(const char *left, const char *right)
{
	return left && right && std::memcmp(left, right, sizeof(TUID)) == 0;
}

class GainView final : public IPlugView {
public:
	tresult PLUGIN_API queryInterface(const TUID interfaceId, void **object) override
	{
		if (!same_id(interfaceId, FUnknown_iid) && !same_id(interfaceId, IPlugView_iid)) {
			*object = nullptr;
			return kNoInterface;
		}
		*object = static_cast<IPlugView *>(this);
		addRef();
		return kResultOk;
	}

	uint32 PLUGIN_API addRef() override { return ++references; }

	uint32 PLUGIN_API release() override
	{
		const uint32 remaining = --references;
		if (!remaining) {
			delete this;
		}
		return remaining;
	}

	tresult PLUGIN_API isPlatformTypeSupported(FIDString type) override
	{
#if SMTG_OS_WINDOWS
		const char *native_type = kPlatformTypeHWND;
#elif SMTG_OS_MACOS
		const char *native_type = kPlatformTypeNSView;
#else
		const char *native_type = kPlatformTypeX11EmbedWindowID;
#endif
		return type && std::strcmp(type, native_type) == 0 ? kResultTrue : kResultFalse;
	}

	tresult PLUGIN_API attached(void *parent, FIDString type) override
	{
		if (!parent || !frame || attached_to_parent || isPlatformTypeSupported(type) != kResultTrue) {
			return kInvalidArgument;
		}
		attached_to_parent = true;
		return kResultOk;
	}

	tresult PLUGIN_API removed() override
	{
		attached_to_parent = false;
		return kResultOk;
	}

	tresult PLUGIN_API onWheel(float) override { return kResultFalse; }
	tresult PLUGIN_API onKeyDown(char16, int16, int16) override { return kResultFalse; }
	tresult PLUGIN_API onKeyUp(char16, int16, int16) override { return kResultFalse; }
	tresult PLUGIN_API onFocus(TBool) override { return kResultOk; }
	tresult PLUGIN_API canResize() override { return kResultFalse; }

	tresult PLUGIN_API getSize(ViewRect *rect) override
	{
		if (!rect) {
			return kInvalidArgument;
		}
		*rect = ViewRect(0, 0, 240, 160);
		return kResultOk;
	}

	tresult PLUGIN_API onSize(ViewRect *) override { return kResultOk; }
	tresult PLUGIN_API checkSizeConstraint(ViewRect *rect) override { return getSize(rect); }

	tresult PLUGIN_API setFrame(IPlugFrame *new_frame) override
	{
		frame = new_frame;
		return kResultOk;
	}

private:
	std::atomic<uint32> references{1};
	IPlugFrame *frame = nullptr;
	bool attached_to_parent = false;
};

class Gain final : public IComponent, public IAudioProcessor, public IEditController {
public:
	explicit Gain(float initial_gain) : gain(initial_gain) {}

	tresult PLUGIN_API queryInterface(const TUID interfaceId, void **object) override
	{
		if (same_id(interfaceId, FUnknown_iid) || same_id(interfaceId, IPluginBase_iid) ||
		    same_id(interfaceId, IComponent_iid)) {
			*object = static_cast<IComponent *>(this);
		} else if (same_id(interfaceId, IAudioProcessor_iid)) {
			*object = static_cast<IAudioProcessor *>(this);
		} else if (same_id(interfaceId, IEditController_iid)) {
			*object = static_cast<IEditController *>(this);
		} else {
			*object = nullptr;
			return kNoInterface;
		}
		addRef();
		return kResultOk;
	}

	uint32 PLUGIN_API addRef() override { return ++references; }

	uint32 PLUGIN_API release() override
	{
		const uint32 remaining = --references;
		if (!remaining) {
			delete this;
		}
		return remaining;
	}

	tresult PLUGIN_API initialize(FUnknown *) override { return kResultOk; }
	tresult PLUGIN_API terminate() override { return kResultOk; }
	tresult PLUGIN_API getControllerClassId(TUID) override { return kResultFalse; }
	tresult PLUGIN_API setIoMode(IoMode) override { return kResultOk; }

	int32 PLUGIN_API getBusCount(MediaType type, BusDirection) override { return type == kAudio ? 1 : 0; }

	tresult PLUGIN_API getBusInfo(MediaType type, BusDirection direction, int32 index, BusInfo &bus) override
	{
		if (type != kAudio || index != 0) {
			return kInvalidArgument;
		}
		bus = {};
		bus.mediaType = type;
		bus.direction = direction;
		bus.channelCount = SpeakerArr::getChannelCount(arrangement);
		bus.busType = kMain;
		bus.flags = BusInfo::kDefaultActive;
		return kResultOk;
	}

	tresult PLUGIN_API getRoutingInfo(RoutingInfo &, RoutingInfo &) override { return kNotImplemented; }
	tresult PLUGIN_API activateBus(MediaType, BusDirection, int32, TBool) override { return kResultOk; }
	tresult PLUGIN_API setActive(TBool) override { return kResultOk; }

	tresult PLUGIN_API setState(IBStream *state) override
	{
		std::array<unsigned char, 12> bytes = {};
		int32 count = 0;
		if (!state || state->read(bytes.data(), static_cast<int32>(bytes.size()), &count) != kResultOk ||
		    count != static_cast<int32>(bytes.size())) {
			return kResultFalse;
		}
		const uint32 bits = static_cast<uint32>(bytes[0]) | (static_cast<uint32>(bytes[1]) << 8) |
				    (static_cast<uint32>(bytes[2]) << 16) | (static_cast<uint32>(bytes[3]) << 24);
		float value = 0.0f;
		std::memcpy(&value, &bits, sizeof(value));
		if (!std::isfinite(value)) {
			return kResultFalse;
		}
		gain.store(value, std::memory_order_relaxed);
		return kResultOk;
	}

	tresult PLUGIN_API getState(IBStream *state) override
	{
		const float value = gain.load(std::memory_order_relaxed);
		uint32 bits = 0;
		std::memcpy(&bits, &value, sizeof(bits));
		std::array<unsigned char, 12> bytes = {};
		for (std::size_t index = 0; index < sizeof(bits); ++index) {
			bytes[index] = static_cast<unsigned char>(bits >> (index * 8));
		}
		int32 count = 0;
		if (!state || state->write(bytes.data(), static_cast<int32>(bytes.size()), &count) != kResultOk ||
		    count != static_cast<int32>(bytes.size())) {
			return kResultFalse;
		}
		return kResultOk;
	}

	tresult PLUGIN_API setBusArrangements(SpeakerArrangement *inputs, int32 num_inputs, SpeakerArrangement *outputs,
					      int32 num_outputs) override
	{
		if (num_inputs != 1 || num_outputs != 1 || inputs[0] != outputs[0] ||
		    (inputs[0] != SpeakerArr::kMono && inputs[0] != SpeakerArr::kStereo)) {
			return kResultFalse;
		}
		arrangement = inputs[0];
		return kResultTrue;
	}

	tresult PLUGIN_API getBusArrangement(BusDirection, int32 index, SpeakerArrangement &result) override
	{
		if (index != 0) {
			return kInvalidArgument;
		}
		result = arrangement;
		return kResultOk;
	}

	tresult PLUGIN_API canProcessSampleSize(int32 size) override
	{
		return size == kSample32 ? kResultTrue : kResultFalse;
	}
	uint32 PLUGIN_API getLatencySamples() override { return 0; }
	tresult PLUGIN_API setupProcessing(ProcessSetup &setup) override
	{
		return canProcessSampleSize(setup.symbolicSampleSize);
	}
	tresult PLUGIN_API setProcessing(TBool) override { return kResultOk; }

	tresult PLUGIN_API process(ProcessData &data) override
	{
		if (data.numSamples == 0) {
			return kResultOk;
		}
		if (data.symbolicSampleSize != kSample32 || data.numInputs != 1 || data.numOutputs != 1 ||
		    data.inputs[0].numChannels != data.outputs[0].numChannels) {
			return kInvalidArgument;
		}

		const float value = gain.load(std::memory_order_relaxed);
		const AudioBusBuffers &input = data.inputs[0];
		AudioBusBuffers &output = data.outputs[0];
		output.silenceFlags = input.silenceFlags;
		for (int32 channel = 0; channel < input.numChannels; ++channel) {
			for (int32 frame = 0; frame < data.numSamples; ++frame) {
				output.channelBuffers32[channel][frame] =
					input.channelBuffers32[channel][frame] * value;
			}
		}
		return kResultOk;
	}

	uint32 PLUGIN_API getTailSamples() override { return kNoTail; }

	tresult PLUGIN_API setComponentState(IBStream *state) override { return setState(state); }
	int32 PLUGIN_API getParameterCount() override { return 0; }
	tresult PLUGIN_API getParameterInfo(int32, ParameterInfo &) override { return kInvalidArgument; }
	tresult PLUGIN_API getParamStringByValue(ParamID, ParamValue, String128) override { return kInvalidArgument; }
	tresult PLUGIN_API getParamValueByString(ParamID, TChar *, ParamValue &) override { return kInvalidArgument; }
	ParamValue PLUGIN_API normalizedParamToPlain(ParamID, ParamValue value) override { return value; }
	ParamValue PLUGIN_API plainParamToNormalized(ParamID, ParamValue value) override { return value; }
	ParamValue PLUGIN_API getParamNormalized(ParamID) override { return 0.0; }
	tresult PLUGIN_API setParamNormalized(ParamID, ParamValue) override { return kInvalidArgument; }
	tresult PLUGIN_API setComponentHandler(IComponentHandler *) override { return kResultOk; }
	IPlugView *PLUGIN_API createView(FIDString name) override
	{
		return name && std::strcmp(name, ViewType::kEditor) == 0 ? new (std::nothrow) GainView : nullptr;
	}

private:
	std::atomic<uint32> references{1};
	std::atomic<float> gain;
	SpeakerArrangement arrangement = SpeakerArr::kStereo;
};

class Factory final : public IPluginFactory2 {
public:
	tresult PLUGIN_API queryInterface(const TUID interfaceId, void **object) override
	{
		if (same_id(interfaceId, FUnknown_iid) || same_id(interfaceId, IPluginFactory_iid) ||
		    same_id(interfaceId, IPluginFactory2_iid)) {
			*object = static_cast<IPluginFactory2 *>(this);
		} else {
			*object = nullptr;
			return kNoInterface;
		}
		addRef();
		return kResultOk;
	}

	uint32 PLUGIN_API addRef() override { return ++references; }
	uint32 PLUGIN_API release() override { return --references; }

	tresult PLUGIN_API getFactoryInfo(PFactoryInfo *info) override
	{
		*info = PFactoryInfo("OBS tests", "", "", PFactoryInfo::kUnicode);
		return kResultOk;
	}

	int32 PLUGIN_API countClasses() override { return 2; }

	tresult PLUGIN_API getClassInfo(int32 index, PClassInfo *info) override
	{
		if (index < 0 || index >= countClasses()) {
			return kInvalidArgument;
		}
		*info = PClassInfo(index == 0 ? gain_id : second_gain_id, PClassInfo::kManyInstances,
				   kVstAudioEffectClass, index == 0 ? "OBS Test Gain" : "OBS Test Gain Alternate");
		return kResultOk;
	}

	tresult PLUGIN_API getClassInfo2(int32 index, PClassInfo2 *info) override
	{
		PClassInfo basic;
		const tresult result = getClassInfo(index, &basic);
		if (result == kResultOk) {
			*info = PClassInfo2(basic.cid, basic.cardinality, basic.category, basic.name, 0, PlugType::kFx,
					    "OBS tests", "1.0.0", kVstVersionString);
		}
		return result;
	}

	tresult PLUGIN_API createInstance(FIDString cid, FIDString interfaceId, void **object) override
	{
		*object = nullptr;
		const bool first = same_id(cid, gain_id);
		if (!first && !same_id(cid, second_gain_id)) {
			return kNoInterface;
		}
		auto *instance = new (std::nothrow) Gain(first ? 1.0f : 0.25f);
		if (!instance) {
			return kOutOfMemory;
		}
		const tresult result = instance->queryInterface(interfaceId, object);
		instance->release();
		return result;
	}

private:
	std::atomic<uint32> references{0};
};

Factory factory;
} // namespace

extern "C" SMTG_EXPORT_SYMBOL IPluginFactory *PLUGIN_API GetPluginFactory()
{
	factory.addRef();
	return &factory;
}

#if SMTG_OS_LINUX
extern "C" SMTG_EXPORT_SYMBOL bool PLUGIN_API ModuleEntry(void *)
{
	return true;
}

extern "C" SMTG_EXPORT_SYMBOL bool PLUGIN_API ModuleExit()
{
	return true;
}
#elif SMTG_OS_WINDOWS
extern "C" SMTG_EXPORT_SYMBOL bool PLUGIN_API InitDll()
{
	return true;
}

extern "C" SMTG_EXPORT_SYMBOL bool PLUGIN_API ExitDll()
{
	return true;
}
#elif SMTG_OS_MACOS
extern "C" SMTG_EXPORT_SYMBOL bool bundleEntry(void *)
{
	return true;
}

extern "C" SMTG_EXPORT_SYMBOL bool bundleExit()
{
	return true;
}
#endif

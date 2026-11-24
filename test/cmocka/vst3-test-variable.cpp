// SPDX-License-Identifier: MIT

#include "plugins/obs-vst3/VST3HostApp.h"
#include "plugins/obs-vst3/VST3Plugin.h"
#include "plugins/obs-vst3/obs-vst3.h"

#include <pluginterfaces/vst/ivstprocesscontext.h>
#include <pluginterfaces/vst/vstspeaker.h>

#include <QCoreApplication>

#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

namespace {
// Match the Rust layouts generated from the pinned SDK.
#define ASSERT_LAYOUT(Type, Size, Align)                                       \
  static_assert(sizeof(Type) == (Size) && alignof(Type) == (Align))
#define ASSERT_FIELD(Type, Field, Offset)                                      \
  static_assert(offsetof(Type, Field) == (Offset))

ASSERT_LAYOUT(TUID, 16, 1);
ASSERT_LAYOUT(AudioBusBuffers, 24, 8);
ASSERT_FIELD(AudioBusBuffers, numChannels, 0);
ASSERT_FIELD(AudioBusBuffers, silenceFlags, 8);
ASSERT_FIELD(AudioBusBuffers, channelBuffers32, 16);
ASSERT_LAYOUT(BusInfo, 276, 4);
ASSERT_FIELD(BusInfo, mediaType, 0);
ASSERT_FIELD(BusInfo, direction, 4);
ASSERT_FIELD(BusInfo, channelCount, 8);
ASSERT_FIELD(BusInfo, name, 12);
ASSERT_FIELD(BusInfo, busType, 268);
ASSERT_FIELD(BusInfo, flags, 272);
ASSERT_LAYOUT(ParameterInfo, 792, 8);
ASSERT_FIELD(ParameterInfo, id, 0);
ASSERT_FIELD(ParameterInfo, title, 4);
ASSERT_FIELD(ParameterInfo, shortTitle, 260);
ASSERT_FIELD(ParameterInfo, units, 516);
ASSERT_FIELD(ParameterInfo, stepCount, 772);
ASSERT_FIELD(ParameterInfo, defaultNormalizedValue, 776);
ASSERT_FIELD(ParameterInfo, unitId, 784);
ASSERT_FIELD(ParameterInfo, flags, 788);
ASSERT_LAYOUT(ProcessSetup, 24, 8);
ASSERT_FIELD(ProcessSetup, processMode, 0);
ASSERT_FIELD(ProcessSetup, symbolicSampleSize, 4);
ASSERT_FIELD(ProcessSetup, maxSamplesPerBlock, 8);
ASSERT_FIELD(ProcessSetup, sampleRate, 16);
ASSERT_LAYOUT(ProcessData, 80, 8);
ASSERT_FIELD(ProcessData, processMode, 0);
ASSERT_FIELD(ProcessData, symbolicSampleSize, 4);
ASSERT_FIELD(ProcessData, numSamples, 8);
ASSERT_FIELD(ProcessData, numInputs, 12);
ASSERT_FIELD(ProcessData, numOutputs, 16);
ASSERT_FIELD(ProcessData, inputs, 24);
ASSERT_FIELD(ProcessData, outputs, 32);
ASSERT_FIELD(ProcessData, inputParameterChanges, 40);
ASSERT_FIELD(ProcessData, outputParameterChanges, 48);
ASSERT_FIELD(ProcessData, inputEvents, 56);
ASSERT_FIELD(ProcessData, outputEvents, 64);
ASSERT_FIELD(ProcessData, processContext, 72);
ASSERT_LAYOUT(ProcessContext, 112, 8);
ASSERT_FIELD(ProcessContext, state, 0);
ASSERT_FIELD(ProcessContext, sampleRate, 8);
ASSERT_FIELD(ProcessContext, projectTimeSamples, 16);
ASSERT_FIELD(ProcessContext, systemTime, 24);
ASSERT_FIELD(ProcessContext, continousTimeSamples, 32);
ASSERT_FIELD(ProcessContext, projectTimeMusic, 40);
ASSERT_FIELD(ProcessContext, barPositionMusic, 48);
ASSERT_FIELD(ProcessContext, cycleStartMusic, 56);
ASSERT_FIELD(ProcessContext, cycleEndMusic, 64);
ASSERT_FIELD(ProcessContext, tempo, 72);
ASSERT_FIELD(ProcessContext, timeSigNumerator, 80);
ASSERT_FIELD(ProcessContext, timeSigDenominator, 84);
ASSERT_FIELD(ProcessContext, chord, 88);
ASSERT_FIELD(ProcessContext, smpteOffsetSubframes, 92);
ASSERT_FIELD(ProcessContext, frameRate, 96);
ASSERT_FIELD(ProcessContext, samplesToNextClock, 104);

#undef ASSERT_LAYOUT
#undef ASSERT_FIELD

constexpr int max_frames = 64;
constexpr char gain_class[] = "57B71384C2EE4713B586F2FD7FDB08A1";
constexpr char missing_class[] = "00000000000000000000000000000000";

bool verify_gain(VST3HostApp &host, const char *bundle)
{
	Vst3AudioData filter{};
	VST3Plugin plugin;
	plugin.hostContext = &host;
	plugin.filterData = &filter;
	if (!plugin.init(gain_class, bundle, 48000, max_frames, SpeakerArr::kStereo)) {
		return false;
	}

	const std::vector<uint8_t> gain_state = {0, 0, 0, 0x3f, 0, 0, 0, 0, 0, 0, 0, 0};
	if (!plugin.loadStates(gain_state, {})) {
		return false;
	}

	std::array<std::array<float, max_frames>, 2> samples = {};
	float *planes[] = {samples[0].data(), samples[1].data()};
	uint32_t random = 0x5536;
	uint32_t hash = 2166136261;
	plugin.setProcessing(true);
	for (const size_t frames : {1, 11, 64, 13, 37, 2}) {
		for (auto &channel : samples) {
			for (size_t frame = 0; frame < frames; ++frame) {
				random = random * 1664525u + 1013904223u;
				channel[frame] = static_cast<float>((random >> 16) & 255) / 64.0f - 2.0f;
			}
		}

		const auto input = samples;
		if (!plugin.processBlock(planes, samples.size(), nullptr, 0, frames)) {
			return false;
		}
		for (size_t channel = 0; channel < samples.size(); ++channel) {
			for (size_t frame = 0; frame < frames; ++frame) {
				if (samples[channel][frame] != input[channel][frame] * 0.5f) {
					return false;
				}
				uint32_t bits = 0;
				std::memcpy(&bits, &samples[channel][frame], sizeof(bits));
				hash = (hash ^ bits) * 16777619u;
			}
		}

		std::vector<uint8_t> component;
		std::vector<uint8_t> controller;
		if (!plugin.saveStates(component, controller) || component != gain_state || controller != gain_state) {
			return false;
		}

		plugin.setProcessing(false);
		plugin.setProcessing(true);
	}

	plugin.setProcessing(false);
	plugin.deactivateComponent();
	std::printf("VST3_PARITY %08x\n", hash);
	return true;
}

bool rejects_configuration(VST3HostApp &host, const char *bundle, const char *class_id, SpeakerArrangement layout)
{
	Vst3AudioData filter{};
	VST3Plugin plugin;
	plugin.hostContext = &host;
	plugin.filterData = &filter;
	return !plugin.init(class_id, bundle, 48000, max_frames, layout);
}
} // namespace

int main(int argc, char **argv)
{
	QCoreApplication app(argc, argv);
	if (argc != 2) {
		return 2;
	}

	VST3HostApp host(VST3Backend::X11);
	const bool passed = rejects_configuration(host, argv[1], missing_class, SpeakerArr::kStereo) &&
			    rejects_configuration(host, argv[1], gain_class, SpeakerArr::k51) &&
			    verify_gain(host, argv[1]) && verify_gain(host, argv[1]);
	return passed ? 0 : 1;
}

#include "plugins/obs-vst3/build/obs-vst3.core.pch.hpp"

#ifdef OBS_VST3_RUST_HOST
VST3Plugin::VST3Plugin() : hostContext(getHostApp())
{
	vst3_prepare_plugin_environment();
}

VST3Plugin::~VST3Plugin()
{
	setProcessing(false);
	deactivateComponent();
	destroyEditor();
	vst3_rust_host_destroy(rustHost);
}

void VST3Plugin::deactivateComponent() const
{
	vst3_rust_host_deactivate(rustHost);
}

bool VST3Plugin::init(const std::string &classId, const std::string &pluginPath, int sampleRate, int blockFrames,
		      SpeakerArrangement arrangement)
{
	if (!hostContext) {
		return false;
	}

	path = pluginPath;
	rustHost = vst3_rust_host_create(pluginPath.c_str(), classId.c_str(), sampleRate, blockFrames, arrangement,
					 hostContext->getFUnknown());
	if (!rustHost) {
		infovst3plugin("Could not initialize VST3 Audio Module Class %s", classId.c_str());
		return false;
	}

	name = vst3_rust_host_name(rustHost);
	sidechainNumberChannels = static_cast<int>(vst3_rust_host_sidechain_channels(rustHost));
	numEnabledOutputAudioBuses = 1;
	return true;
}

void VST3Plugin::setProcessing(bool processing) const
{
	vst3_rust_host_set_processing(rustHost, processing);
}

bool VST3Plugin::processBlock(float *const *main, size_t channels, const float *const *sidechain,
			      size_t sidechainFrames, size_t frames)
{
	bool guiPending = false;
	const bool processed = vst3_rust_host_process(rustHost, main, channels, sidechain, sidechainFrames, frames,
						      static_cast<int64_t>(os_gettime_ns()), &guiPending);

	if (guiPending && !uiDrainScheduled.exchange(true, std::memory_order_acq_rel)) {
		if (QObject *receiver = QCoreApplication::instance()) {
			QPointer<VST3Plugin> self(this);
			QMetaObject::invokeMethod(
				receiver,
				[self] {
					if (auto *plugin = self.data()) {
						plugin->drainDspToGui();
					}
				},
				Qt::QueuedConnection);
		} else {
			uiDrainScheduled.store(false, std::memory_order_release);
		}
	}
	return processed;
}

void VST3Plugin::drainDspToGui()
{
	uiDrainScheduled.store(false, std::memory_order_release);
	vst3_rust_host_drain_gui(rustHost);
}

bool VST3Plugin::saveStates(std::vector<uint8_t> &component, std::vector<uint8_t> &controller) const
{
	component.clear();
	controller.clear();
	std::unique_ptr<Vst3RustState, decltype(&vst3_rust_state_destroy)> state(vst3_rust_host_save(rustHost),
										 vst3_rust_state_destroy);
	if (!state) {
		return false;
	}
	size_t size = 0;
	const auto *bytes = vst3_rust_state_data(state.get(), false, &size);
	if (size) {
		component.assign(bytes, bytes + size);
	}
	bytes = vst3_rust_state_data(state.get(), true, &size);
	if (size) {
		controller.assign(bytes, bytes + size);
	}
	return true;
}

bool VST3Plugin::loadStates(const std::vector<uint8_t> &component, const std::vector<uint8_t> &controller) const
{
	return vst3_rust_host_restore(rustHost, component.data(), component.size(), controller.data(),
				      controller.size());
}
#endif

#pragma once

#include <cstdint>

#include <util/platform.h>

/* How long quitting OBS waits for an active recording to finish stopping. */
constexpr uint64_t RecordingShutdownTimeoutMs = 5000;

/* Called by OBSBasic::closeWindow() before the outputs are destroyed.
 *
 * Destroying an active output force-stops it, which drops every frame the
 * encoder still holds (x264's lookahead and B-frames). So an active
 * recording is stopped the way Stop Recording stops it, and the quit waits
 * until it has stopped, at most timeout_ms. A recording that has not
 * stopped by then is force-stopped, as before, so quitting never hangs on
 * it. Returns false when the recording had to be force-stopped. */
template<typename Handler> bool StopRecordingForShutdown(Handler *handler, uint64_t timeout_ms)
{
	if (!handler || !handler->RecordingActive()) {
		return true;
	}

	handler->StopRecording(false);

	const uint64_t deadline = os_gettime_ns() + timeout_ms * 1000000ULL;
	while (handler->RecordingActive()) {
		if (os_gettime_ns() >= deadline) {
			handler->StopRecording(true);
			return false;
		}
		os_sleep_ms(10);
	}

	return true;
}

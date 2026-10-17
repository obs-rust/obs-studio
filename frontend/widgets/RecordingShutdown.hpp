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
 * it. Returns false when the recording had to be force-stopped.
 *
 * wait(timeout_ms) does the waiting and returns whether the recording
 * stopped in time. closeWindow() passes WaitForOutputToStop()
 * (RecordingShutdownWait.hpp), which keeps the UI event loop running on the
 * output's stop signals instead of sleeping on the UI thread (#1283). */
template<typename Handler, typename Wait>
bool StopRecordingForShutdown(Handler *handler, uint64_t timeout_ms, Wait &&wait)
{
	if (!handler || !handler->RecordingActive()) {
		return true;
	}

	handler->StopRecording(false);

	if (wait(timeout_ms) || !handler->RecordingActive()) {
		return true;
	}

	handler->StopRecording(true);
	return false;
}

/* The same, waiting by polling RecordingActive(): for callers off the UI
 * thread, where blocking is harmless. */
template<typename Handler> bool StopRecordingForShutdown(Handler *handler, uint64_t timeout_ms)
{
	return StopRecordingForShutdown(handler, timeout_ms, [handler](uint64_t ms) {
		const uint64_t deadline = os_gettime_ns() + ms * 1000000ULL;
		while (handler->RecordingActive()) {
			if (os_gettime_ns() >= deadline) {
				return false;
			}
			os_sleep_ms(10);
		}
		return true;
	});
}

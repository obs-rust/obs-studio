#pragma once

#include <cstdint>
#include <functional>

#include <callback/signal.h>
#include <util/platform.h>

/* Waits, at most timeout_ms, for an output that was asked to stop to finish
 * stopping, as StopRecordingForShutdown()'s wait step. Returns whether it
 * stopped. active() reports whether the output is still active. */
inline bool WaitForOutputToStop(signal_handler_t *output_signals, uint64_t timeout_ms,
				const std::function<bool()> &active)
{
	(void)output_signals;
	const uint64_t deadline = os_gettime_ns() + timeout_ms * 1000000ULL;
	while (active()) {
		if (os_gettime_ns() >= deadline) {
			return false;
		}
		os_sleep_ms(10);
	}
	return true;
}

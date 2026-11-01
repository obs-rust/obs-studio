#pragma once

/* OBS Virtual Microphone on macOS (obs-rust/obs-studio#3184): the device
 * clock the HAL plug-in reports to coreaudiod. The device runs at `rate`
 * frames per second from `anchor` (host time when IO started); every
 * `period` frames it publishes a zero time stamp. Plain C so the cmocka test
 * runs on every host. */

#include <stdint.h>

struct vmic_zero_timestamp {
	double sample_time;
	uint64_t host_time;
	uint64_t seed;
};

/* The latest zero time stamp at host time `now`. `ticks_per_frame` is host
 * ticks per sample frame (mach_timebase converted). */
static inline struct vmic_zero_timestamp vmic_clock_zero(uint64_t anchor, uint64_t now, double ticks_per_frame,
							 uint32_t period)
{
	struct vmic_zero_timestamp ts = {0, anchor, 1};
	double ticks_per_period = ticks_per_frame * period;
	if (now <= anchor || ticks_per_period <= 0) {
		return ts;
	}
	uint64_t cycles = (uint64_t)((double)(now - anchor) / ticks_per_period);
	ts.sample_time = (double)cycles * period;
	ts.host_time = anchor + (uint64_t)((double)cycles * ticks_per_period);
	return ts;
}

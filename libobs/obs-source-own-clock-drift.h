/******************************************************************************
    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#pragma once

/* The clock drift error of audio on a clock of its own (#4182), for
 * source_output_audio_data() in obs-source-audio-output.c.h. The same filter
 * as rust/obs-source/src/own_clock_drift.rs, which the Rust audio path runs on
 * this struct; the parity tests compare the two.
 *
 * The error is the end of the buffered audio against each appended packet's
 * arrival: a minimum over a sliding window of 1 s (8 buckets of 125 ms), so
 * late packets do not move it, less a baseline, the minimum over the first
 * second. A positive error means the buffer holds more audio than at the
 * start (a sender clock faster than the system clock), the sign
 * audio_resampler_set_compensation_error() expects.
 *
 * All zeroes is a filter with no samples. Private to libobs, not EXPORT. */

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "util/util_uint64.h"

#define OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKET_NS 125000000ULL
#define OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKETS 8
#define OBS_SOURCE_OWN_CLOCK_DRIFT_WINDOW_NS (OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKET_NS * OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKETS)

struct obs_source_own_clock_drift_bucket {
	/* os_time / OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKET_NS */
	uint64_t id;
	/* the smallest gap in the bucket */
	int64_t min;
	bool set;
};

struct obs_source_own_clock_drift {
	/* the arrival of the first sample, when start_set */
	uint64_t start;
	/* the smallest gap in the start window so far, when start_set */
	int64_t start_min;
	/* the start window's minimum, when baseline_set */
	int64_t baseline;
	bool start_set;
	bool baseline_set;
	/* each bucket in the slot of its number modulo the bucket count */
	struct obs_source_own_clock_drift_bucket buckets[OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKETS];
};

/* How far the end of the buffered audio is ahead of @os_time, in ns:
 * @audio_ts plus @buffered_frames at @rate, less @os_time. Saturates at the
 * int64_t range; a @rate of 0 counts the frames as no time. */
static inline int64_t obs_source_own_clock_drift_gap(uint64_t audio_ts, uint64_t buffered_frames, uint32_t rate,
						     uint64_t os_time)
{
	uint64_t frames_ns = rate ? util_mul_div64(buffered_frames, 1000000000ULL, rate) : 0;
	uint64_t end = audio_ts + frames_ns;
	bool wrapped = end < audio_ts;

	if (wrapped) {
		/* the end is 2^64 + end: the gap is at least 2^64 - os_time */
		uint64_t to_wrap = 0 - os_time;
		if (os_time == 0 || to_wrap > (uint64_t)INT64_MAX)
			return INT64_MAX;
		uint64_t gap = to_wrap + end;
		if (gap < to_wrap || gap > (uint64_t)INT64_MAX)
			return INT64_MAX;
		return (int64_t)gap;
	}
	if (end >= os_time) {
		uint64_t gap = end - os_time;
		return gap > (uint64_t)INT64_MAX ? INT64_MAX : (int64_t)gap;
	}
	uint64_t behind = os_time - end;
	if (behind > (uint64_t)INT64_MAX)
		return INT64_MIN;
	return -(int64_t)behind;
}

static inline int64_t obs_source_own_clock_drift_sat_sub(int64_t a, int64_t b)
{
	if (b > 0 && a < INT64_MIN + b)
		return INT64_MIN;
	if (b < 0 && a > INT64_MAX + b)
		return INT64_MAX;
	return a - b;
}

/* How much more audio a source on its own clock holds than right after a
 * packet placed at its arrival: the gap before appending less the offset such
 * a packet is placed at (sync_offset - resample_offset), each step saturating
 * at the int64_t range. */
static inline int64_t obs_source_own_clock_drift_excess(int64_t gap, int64_t sync_offset, uint64_t resample_offset)
{
	int64_t resample = resample_offset > (uint64_t)INT64_MAX ? INT64_MAX : (int64_t)resample_offset;
	return obs_source_own_clock_drift_sat_sub(obs_source_own_clock_drift_sat_sub(gap, sync_offset), -resample);
}

static inline void obs_source_own_clock_drift_reset(struct obs_source_own_clock_drift *f)
{
	memset(f, 0, sizeof(*f));
}

static inline void obs_source_own_clock_drift_insert(struct obs_source_own_clock_drift *f, uint64_t id, int64_t gap)
{
	struct obs_source_own_clock_drift_bucket *b = &f->buckets[id % OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKETS];

	if (b->set && b->id == id) {
		if (gap < b->min)
			b->min = gap;
	} else if (b->set && b->id > id) {
		/* a newer bucket in the same slot stays */
	} else {
		b->id = id;
		b->min = gap;
		b->set = true;
	}
}

/* Adds the gap of a packet that arrived at @os_time. Returns true and sets
 * @error to the sliding minimum less the baseline; false until a full window
 * has passed since the first sample, and for a sample older than the
 * window. */
static inline bool obs_source_own_clock_drift_sample(struct obs_source_own_clock_drift *f, uint64_t os_time,
						     int64_t gap, int64_t *error)
{
	uint64_t id = os_time / OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKET_NS;
	bool have_newest = false;
	uint64_t newest = 0;

	for (size_t i = 0; i < OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKETS; i++) {
		if (f->buckets[i].set && (!have_newest || f->buckets[i].id > newest)) {
			newest = f->buckets[i].id;
			have_newest = true;
		}
	}
	if (have_newest) {
		uint64_t reach = id > UINT64_MAX - OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKETS
					 ? UINT64_MAX
					 : id + OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKETS;
		if (reach <= newest)
			return false;
	}

	if (!f->start_set) {
		f->start = os_time;
		f->start_min = INT64_MAX;
		f->start_set = true;
	}
	if (!f->baseline_set) {
		uint64_t since = os_time > f->start ? os_time - f->start : 0;
		if (since < OBS_SOURCE_OWN_CLOCK_DRIFT_WINDOW_NS) {
			if (gap < f->start_min)
				f->start_min = gap;
			obs_source_own_clock_drift_insert(f, id, gap);
			return false;
		}
		f->baseline = f->start_min;
		f->baseline_set = true;
	}

	obs_source_own_clock_drift_insert(f, id, gap);

	uint64_t current = have_newest && newest > id ? newest : id;
	int64_t min = gap;
	bool have_min = false;
	for (size_t i = 0; i < OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKETS; i++) {
		const struct obs_source_own_clock_drift_bucket *b = &f->buckets[i];
		if (!b->set || b->id > current || current - b->id >= OBS_SOURCE_OWN_CLOCK_DRIFT_BUCKETS)
			continue;
		if (!have_min || b->min < min) {
			min = b->min;
			have_min = true;
		}
	}
	*error = obs_source_own_clock_drift_sat_sub(min, f->baseline);
	return true;
}

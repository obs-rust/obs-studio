/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

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

#include "libobs/build/libobs.pch.h"

/* The arithmetic of the audio tick, moved from obs-audio.c (#1903) before its
 * port to obs-core: the frame offset of a time in the tick, the mix of a root
 * source's audio into the output mixes, and the buffering steps. audio_callback()
 * in obs-audio.c calls them. Declared in obs-internal.h, not exported. */

#define DEBUG_AUDIO 0

size_t convert_time_to_frames(size_t sample_rate, uint64_t t)
{
	return (size_t)util_mul_div64(t, sample_rate, 1000000000ULL);
}

void mix_audio(struct audio_output_data *mixes, obs_source_t *source, size_t channels, size_t sample_rate,
	       struct ts_info *ts)
{
	size_t total_floats = AUDIO_OUTPUT_FRAMES;
	size_t start_point = 0;

	if (source->audio_ts < ts->start || ts->end <= source->audio_ts)
		return;

	if (source->audio_ts != ts->start) {
		start_point = convert_time_to_frames(sample_rate, source->audio_ts - ts->start);
		if (start_point == AUDIO_OUTPUT_FRAMES)
			return;

		total_floats -= start_point;
	}

	for (size_t mix_idx = 0; mix_idx < MAX_AUDIO_MIXES; mix_idx++) {
		for (size_t ch = 0; ch < channels; ch++) {
			register float *mix = mixes[mix_idx].data[ch];
			register float *aud = source->audio_output_buf[mix_idx][ch];
			register float *end;

			mix += start_point;
			end = aud + total_floats;

			while (aud < end)
				*(mix++) += *(aud++);
		}
	}
}

bool audio_buffering_maxed(struct obs_core_audio *audio)
{
	return audio->total_buffering_ticks == audio->max_buffering_ticks;
}

void set_fixed_audio_buffering(struct obs_core_audio *audio, size_t sample_rate, struct ts_info *ts)
{
	struct ts_info new_ts;
	size_t total_ms;
	int ticks;

	if (audio_buffering_maxed(audio))
		return;

	if (!audio->buffering_wait_ticks)
		audio->buffered_ts = ts->start;

	ticks = audio->max_buffering_ticks - audio->total_buffering_ticks;
	audio->total_buffering_ticks += ticks;

	total_ms = audio->total_buffering_ticks * AUDIO_OUTPUT_FRAMES * 1000 / sample_rate;

	blog(LOG_INFO,
	     "Enabling fixed audio buffering, total "
	     "audio buffering is now %d milliseconds",
	     (int)total_ms);

	new_ts.start =
		audio->buffered_ts - audio_frames_to_ns(sample_rate, audio->buffering_wait_ticks * AUDIO_OUTPUT_FRAMES);

	while (ticks--) {
		const uint64_t cur_ticks = ++audio->buffering_wait_ticks;

		new_ts.end = new_ts.start;
		new_ts.start = audio->buffered_ts - audio_frames_to_ns(sample_rate, cur_ticks * AUDIO_OUTPUT_FRAMES);

#if DEBUG_AUDIO == 1
		blog(LOG_DEBUG, "add buffered ts: %" PRIu64 "-%" PRIu64, new_ts.start, new_ts.end);
#endif

		deque_push_front(&audio->buffered_timestamps, &new_ts, sizeof(new_ts));
	}

	*ts = new_ts;
}

void add_audio_buffering(struct obs_core_audio *audio, size_t sample_rate, struct ts_info *ts, uint64_t min_ts,
			 const char *buffering_name)
{
	/* A sub-sample offset adds no ticks, so the loop may leave end untouched. */
	struct ts_info new_ts = *ts;
	uint64_t offset;
	uint64_t frames;
	size_t total_ms;
	size_t ms;
	int ticks;

	if (audio_buffering_maxed(audio))
		return;

	if (!audio->buffering_wait_ticks)
		audio->buffered_ts = ts->start;

	offset = ts->start - min_ts;
	frames = ns_to_audio_frames(sample_rate, offset);
	ticks = (int)((frames + AUDIO_OUTPUT_FRAMES - 1) / AUDIO_OUTPUT_FRAMES);

	audio->total_buffering_ticks += ticks;

	if (audio->total_buffering_ticks >= audio->max_buffering_ticks) {
		ticks -= audio->total_buffering_ticks - audio->max_buffering_ticks;
		audio->total_buffering_ticks = audio->max_buffering_ticks;
		blog(LOG_WARNING, "Max audio buffering reached!");
	}

	ms = ticks * AUDIO_OUTPUT_FRAMES * 1000 / sample_rate;
	total_ms = audio->total_buffering_ticks * AUDIO_OUTPUT_FRAMES * 1000 / sample_rate;

	blog(LOG_INFO,
	     "adding %d milliseconds of audio buffering, total "
	     "audio buffering is now %d milliseconds"
	     " (source: %s)\n",
	     (int)ms, (int)total_ms, buffering_name);
#if DEBUG_AUDIO == 1
	blog(LOG_DEBUG,
	     "min_ts (%" PRIu64 ") < start timestamp "
	     "(%" PRIu64 ")",
	     min_ts, ts->start);
	blog(LOG_DEBUG, "old buffered ts: %" PRIu64 "-%" PRIu64, ts->start, ts->end);
#endif

	new_ts.start =
		audio->buffered_ts - audio_frames_to_ns(sample_rate, audio->buffering_wait_ticks * AUDIO_OUTPUT_FRAMES);

	while (ticks--) {
		const uint64_t cur_ticks = ++audio->buffering_wait_ticks;

		new_ts.end = new_ts.start;
		new_ts.start = audio->buffered_ts - audio_frames_to_ns(sample_rate, cur_ticks * AUDIO_OUTPUT_FRAMES);

#if DEBUG_AUDIO == 1
		blog(LOG_DEBUG, "add buffered ts: %" PRIu64 "-%" PRIu64, new_ts.start, new_ts.end);
#endif

		deque_push_front(&audio->buffered_timestamps, &new_ts, sizeof(new_ts));
	}

	*ts = new_ts;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef DEBUG_AUDIO

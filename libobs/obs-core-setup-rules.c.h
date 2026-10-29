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

#include "libobs/build/libobs-ported-core-setup-rules.pch.h"

/* Setup rules, moved from obs_reset_video(), obs_reset_audio2(),
 * obs_set_cmdline_args(), obs_find_data_file(), obs_remove_data_path(),
 * obs_get_latest_input_type_id(), check_all_aoc_sources(),
 * obs_set_audio_monitoring_device() and obs_get_video_frame_counts() in
 * obs.c: the reset gates, the audio buffering ticks, the command line's
 * copy, the data path match, the latest version, the capture device check,
 * the monitoring device checks and the frame-count rule. Pure: no libobs
 * state. obs.c keeps the reads, the calls, the locks, the copies, the frees
 * and the logging, and passes the values in, one step at a time. Declared
 * in obs-core-setup-rules.h.
 * Ported to rust/obs-core (core_setup_rules): the hybrid libobs drops this
 * file. */

int obs_core_setup_rules_video_gate(enum obs_core_setup_rules_video_step step, bool value)
{
	switch (step) {
	case OBS_CORE_SETUP_RULES_VIDEO_CORE:
		return value ? OBS_CORE_SETUP_RULES_VIDEO_GO : OBS_CORE_SETUP_RULES_VIDEO_FAIL;
	/* don't allow changing of video settings if active, or about to be. */
	case OBS_CORE_SETUP_RULES_VIDEO_ACTIVE:
	case OBS_CORE_SETUP_RULES_VIDEO_STARTING:
		return value ? OBS_CORE_SETUP_RULES_VIDEO_CURRENTLY_ACTIVE : OBS_CORE_SETUP_RULES_VIDEO_GO;
	case OBS_CORE_SETUP_RULES_VIDEO_OUTPUT_SIZE:
	case OBS_CORE_SETUP_RULES_VIDEO_BASE_SIZE:
		return value ? OBS_CORE_SETUP_RULES_VIDEO_GO : OBS_CORE_SETUP_RULES_VIDEO_INVALID_PARAM;
	}
	return OBS_CORE_SETUP_RULES_VIDEO_GO;
}

/* don't allow changing of audio settings if active. */
bool obs_core_setup_rules_audio_refused(enum obs_core_setup_rules_audio_step step, bool value)
{
	switch (step) {
	case OBS_CORE_SETUP_RULES_AUDIO_CORE:
		return !value;
	case OBS_CORE_SETUP_RULES_AUDIO_ACTIVE:
		return value;
	}
	return false;
}

bool obs_core_setup_rules_buffer_set(uint32_t max_buffering_ms)
{
	return max_buffering_ms != 0;
}

/* The frames in max_buffering_ms, rounded up to whole ticks. The product is
 * a uint32_t, so it wraps above UINT32_MAX. */
uint32_t obs_core_setup_rules_buffer_ticks(uint32_t max_buffering_ms, uint32_t samples_per_sec)
{
	uint32_t max_frames = max_buffering_ms * samples_per_sec / 1000;
	max_frames += (OBS_CORE_SETUP_RULES_AUDIO_OUTPUT_FRAMES - 1);
	return max_frames / OBS_CORE_SETUP_RULES_AUDIO_OUTPUT_FRAMES;
}

uint32_t obs_core_setup_rules_buffer_default_ticks(void)
{
	return OBS_CORE_SETUP_RULES_DEFAULT_TICKS;
}

/* Once argc is set (non-zero) we shouldn't call again */
bool obs_core_setup_rules_cmdline_set(int stored_argc)
{
	return stored_argc != 0;
}

size_t obs_core_setup_rules_cmdline_len(size_t len, size_t arg_len)
{
	return len + arg_len + 1;
}

size_t obs_core_setup_rules_cmdline_table(int argc, size_t ptr_size)
{
	return ptr_size * ((size_t)argc + 1);
}

bool obs_core_setup_rules_data_file_found(bool found)
{
	return found;
}

bool obs_core_setup_rules_data_path_match(int cmp)
{
	return cmp == 0;
}

bool obs_core_setup_rules_latest_newer(uint32_t version, int best)
{
	return (int)version > best;
}

bool obs_core_setup_rules_aoc_check(uint32_t output_flags)
{
	return (output_flags & OBS_CORE_SETUP_RULES_DO_NOT_SELF_MONITOR) != 0;
}

bool obs_core_setup_rules_monitor_missing(bool present)
{
	return !present;
}

bool obs_core_setup_rules_monitor_same(int cmp)
{
	return cmp == 0;
}

/* An encoder that rescales on the GPU encodes from an encoder-only mix of
 * the same view (maybe_set_up_gpu_rescale() in obs-encoder.c), and that
 * mix's video_t counts its frames. */
enum obs_core_setup_rules_mix obs_core_setup_rules_mix_step(enum obs_core_setup_rules_mix step, bool value)
{
	switch (step) {
	case OBS_CORE_SETUP_RULES_MIX_MAIN:
		return value ? OBS_CORE_SETUP_RULES_MIX_VIDEO : OBS_CORE_SETUP_RULES_MIX_ENCODER_ONLY;
	case OBS_CORE_SETUP_RULES_MIX_ENCODER_ONLY:
		return value ? OBS_CORE_SETUP_RULES_MIX_VIEW : OBS_CORE_SETUP_RULES_MIX_SKIP;
	case OBS_CORE_SETUP_RULES_MIX_VIEW:
		return value ? OBS_CORE_SETUP_RULES_MIX_VIDEO : OBS_CORE_SETUP_RULES_MIX_SKIP;
	case OBS_CORE_SETUP_RULES_MIX_VIDEO:
		return value ? OBS_CORE_SETUP_RULES_MIX_COUNT : OBS_CORE_SETUP_RULES_MIX_SKIP;
	case OBS_CORE_SETUP_RULES_MIX_COUNT:
	case OBS_CORE_SETUP_RULES_MIX_SKIP:
		break;
	}
	return step;
}

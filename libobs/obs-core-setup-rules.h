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

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The setup rules of obs.c, on plain values (obs-core-setup-rules.c). Each
 * one is asked where the C decided it, from the value the caller reads at
 * that point, so nothing is read or called that was not before:
 * - obs_core_setup_rules_video_gate(): obs_reset_video()'s gates, one step
 *   at a time (no core, video active, outputs starting, the output size,
 *   the base size): the OBS_VIDEO_* code to return, or
 *   OBS_CORE_SETUP_RULES_VIDEO_GO to go on to the next step;
 * - obs_core_setup_rules_audio_refused(): obs_reset_audio2()'s gates (no
 *   core, then an active audio output, asked only when there is one);
 * - obs_core_setup_rules_buffer_set(), obs_core_setup_rules_buffer_ticks(),
 *   obs_core_setup_rules_buffer_default_ticks(): the audio buffering in
 *   ticks of AUDIO_OUTPUT_FRAMES: a max_buffering_ms of 0 takes the
 *   default, any other is rounded up from ms * rate / 1000 frames, in
 *   uint32_t (the product wraps);
 * - obs_core_setup_rules_cmdline_set(): the command line was set already
 *   (its stored argc is not 0), so a new one is ignored;
 * - obs_core_setup_rules_cmdline_len(), obs_core_setup_rules_cmdline_table():
 *   the size of the copy's strings (each with its terminator), and of its
 *   table of argc + 1 pointers, which the strings follow;
 * - obs_core_setup_rules_data_file_found(): obs_find_data_file() stops at
 *   a hit, asked after the libobs data directory and after each added path
 *   in order, so the first place that has the file wins;
 * - obs_core_setup_rules_data_path_match(): a data path equal to the one
 *   removed (dstr_cmp() of 0); the first one is removed;
 * - obs_core_setup_rules_latest_newer(): a source type's version is above
 *   the best so far, compared as int (the best starts at
 *   OBS_CORE_SETUP_RULES_LATEST_NONE), so the first of equal versions wins;
 * - obs_core_setup_rules_aoc_check(): a source whose output flags ask
 *   check_all_aoc_sources() to check its capture device;
 * - obs_core_setup_rules_monitor_missing(): obs_set_audio_monitoring_device()
 *   refuses: asked for the name, the id, the name's first character, the
 *   id's first character and the monitoring backend, in turn;
 * - obs_core_setup_rules_monitor_same(): the device id equals the current
 *   one (strcmp() of 0), so nothing is reset;
 * - obs_core_setup_rules_mix_step(): obs_get_video_frame_counts()'s rule,
 *   one read at a time: a mix counts when it is the main mix, or an
 *   encoder-only mix of the main mix's view, and has a video output.
 *
 * obs.c keeps every lock, every read and call (obs_video_active(),
 * outputs_starting(), audio_output_active(), find_libobs_data_file(),
 * check_path(), dstr_cmp(), strcmp(), strlen(),
 * obs_audio_monitoring_available(), the frame counters), the fps gate, every
 * allocation, copy, free and signal, every divide by a variable and every log
 * message. Nothing here takes a lock, calls into libobs or keeps state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs.c checks them against obs-defs.h and obs-source.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_VIDEO_SUCCESS, OBS_VIDEO_FAIL, OBS_VIDEO_INVALID_PARAM,
 * OBS_VIDEO_CURRENTLY_ACTIVE */
#define OBS_CORE_SETUP_RULES_VIDEO_GO 0
#define OBS_CORE_SETUP_RULES_VIDEO_FAIL -1
#define OBS_CORE_SETUP_RULES_VIDEO_INVALID_PARAM -3
#define OBS_CORE_SETUP_RULES_VIDEO_CURRENTLY_ACTIVE -4
/* OBS_SOURCE_DO_NOT_SELF_MONITOR */
#define OBS_CORE_SETUP_RULES_DO_NOT_SELF_MONITOR (1u << 9)
/* AUDIO_OUTPUT_FRAMES, and the buffering obs_reset_audio2() takes for a
 * max_buffering_ms of 0, in ticks */
#define OBS_CORE_SETUP_RULES_AUDIO_OUTPUT_FRAMES 1024
#define OBS_CORE_SETUP_RULES_DEFAULT_TICKS 45
/* The version obs_get_latest_input_type_id() starts from */
#define OBS_CORE_SETUP_RULES_LATEST_NONE -1

/* The steps of obs_reset_video()'s gates, in order, with the value each
 * one is asked with. */
enum obs_core_setup_rules_video_step {
	OBS_CORE_SETUP_RULES_VIDEO_CORE,        /* obs != NULL */
	OBS_CORE_SETUP_RULES_VIDEO_ACTIVE,      /* obs_video_active() */
	OBS_CORE_SETUP_RULES_VIDEO_STARTING,    /* outputs_starting() */
	OBS_CORE_SETUP_RULES_VIDEO_OUTPUT_SIZE, /* the output size is valid */
	OBS_CORE_SETUP_RULES_VIDEO_BASE_SIZE,   /* the base size is valid */
};

/* The steps of obs_reset_audio2()'s gates, in order. */
enum obs_core_setup_rules_audio_step {
	OBS_CORE_SETUP_RULES_AUDIO_CORE,   /* obs != NULL */
	OBS_CORE_SETUP_RULES_AUDIO_ACTIVE, /* audio_output_active() */
};

/* What obs_get_video_frame_counts() reads next about a mix (each value is
 * asked with that read), or what it does with the mix. The steps only go
 * forward, in this order. */
enum obs_core_setup_rules_mix {
	OBS_CORE_SETUP_RULES_MIX_MAIN,         /* mix == main_mix */
	OBS_CORE_SETUP_RULES_MIX_ENCODER_ONLY, /* mix->encoder_only_mix */
	OBS_CORE_SETUP_RULES_MIX_VIEW,         /* mix->view == main_mix->view */
	OBS_CORE_SETUP_RULES_MIX_VIDEO,        /* mix->video != NULL */
	OBS_CORE_SETUP_RULES_MIX_COUNT,
	OBS_CORE_SETUP_RULES_MIX_SKIP,
};

int obs_core_setup_rules_video_gate(enum obs_core_setup_rules_video_step step, bool value);
bool obs_core_setup_rules_audio_refused(enum obs_core_setup_rules_audio_step step, bool value);
bool obs_core_setup_rules_buffer_set(uint32_t max_buffering_ms);
uint32_t obs_core_setup_rules_buffer_ticks(uint32_t max_buffering_ms, uint32_t samples_per_sec);
uint32_t obs_core_setup_rules_buffer_default_ticks(void);
bool obs_core_setup_rules_cmdline_set(int stored_argc);
size_t obs_core_setup_rules_cmdline_len(size_t len, size_t arg_len);
size_t obs_core_setup_rules_cmdline_table(int argc, size_t ptr_size);
bool obs_core_setup_rules_data_file_found(bool found);
bool obs_core_setup_rules_data_path_match(int cmp);
bool obs_core_setup_rules_latest_newer(uint32_t version, int best);
bool obs_core_setup_rules_aoc_check(uint32_t output_flags);
bool obs_core_setup_rules_monitor_missing(bool present);
bool obs_core_setup_rules_monitor_same(int cmp);
enum obs_core_setup_rules_mix obs_core_setup_rules_mix_step(enum obs_core_setup_rules_mix step, bool value);

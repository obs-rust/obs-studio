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
#include <stdint.h>

/* The per-source decisions of the "build audio render order" and "render
 * audio data" steps of the audio tick (obs-audio-render.c), on plain values.
 * The walks over the views and the audio sources, the locks, the refs, the
 * render_order and root_nodes arrays, the output buffers and the logging stay
 * in obs-audio.c, which calls these for each source. Nothing here takes a
 * lock, logs or calls into libobs.
 *
 * obs_audio_render_tree() is push_audio_tree2's choice for one visited
 * source: skip a removed source, add a source that is not in the render
 * order yet, tag an individual audio source (an input with audio, not
 * composite) that is in it already and not tagged yet (it is then mixed as
 * a root node, not through its parents), or nothing.
 *
 * obs_audio_render_silence() is should_silence_monitored_source: a
 * monitored source is silenced while an Audio Output Capture source records
 * the monitoring device, is active and is not muted (by its mute or by a
 * volume within 0.0001 of 0). The capture source itself is not silenced.
 *
 * obs_audio_render_clear_mixes() is clear_audio_output_buf's choice: the
 * mixes (bits below MAX_AUDIO_MIXES) that both the capture source and the
 * source use.
 *
 * obs_audio_render_backwards() is the render loop's check for a source that
 * has gone backward in time while the buffering is at the maximum: a source
 * with audio_render is marked pending, another source has its audio dropped
 * (ignore_audio) and is rendered again, otherwise nothing.
 *
 * The source type and the two output flags are copies of the obs-source.h
 * values, checked by _Static_assert in obs-audio.c, so the core compiles
 * without obs.h.
 *
 * Hidden in libobs: not EXPORT. */

#define OBS_AUDIO_RENDER_TYPE_INPUT 0
#define OBS_AUDIO_RENDER_SOURCE_AUDIO (1 << 1)
#define OBS_AUDIO_RENDER_SOURCE_COMPOSITE (1 << 6)

enum obs_audio_tree_choice {
	OBS_AUDIO_TREE_NONE,
	OBS_AUDIO_TREE_SKIP,
	OBS_AUDIO_TREE_ADD,
	OBS_AUDIO_TREE_DUPLICATE,
};

enum obs_audio_backwards_choice {
	OBS_AUDIO_BACKWARDS_NONE,
	OBS_AUDIO_BACKWARDS_PENDING,
	OBS_AUDIO_BACKWARDS_IGNORE,
};

struct obs_audio_monitor_dedup {
	bool set;
	bool active;
	bool muted;
	float volume;
	uint32_t mixers;
};

enum obs_audio_tree_choice obs_audio_render_tree(bool removed, bool present, uint32_t type, uint32_t output_flags,
						 bool duplicated);
bool obs_audio_render_silence(const struct obs_audio_monitor_dedup *dedup, bool monitoring_enabled,
			      bool is_dedup_source);
uint32_t obs_audio_render_clear_mixes(uint32_t dedup_mixers, uint32_t source_mixers);
enum obs_audio_backwards_choice obs_audio_render_backwards(bool maxed, uint64_t audio_ts, uint64_t tick_start,
							   bool audio_render);

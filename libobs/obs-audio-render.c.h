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

#include "libobs/build/libobs-ported-audio-render.pch.h"

/* The per-source decisions of the audio render order and render steps, on
 * plain values, declared in obs-audio-render.h. obs-audio.c walks the tree,
 * holds the locks and refs, keeps render_order and root_nodes and passes the
 * fields in. */

static inline bool obs_audio_render_individual(uint32_t type, uint32_t output_flags)
{
	return type == OBS_AUDIO_RENDER_TYPE_INPUT && (output_flags & OBS_AUDIO_RENDER_SOURCE_AUDIO) &&
	       !(output_flags & OBS_AUDIO_RENDER_SOURCE_COMPOSITE);
}

enum obs_audio_tree_choice obs_audio_render_tree(bool removed, bool present, uint32_t type, uint32_t output_flags,
						 bool duplicated)
{
	if (removed)
		return OBS_AUDIO_TREE_SKIP;
	if (!present)
		return OBS_AUDIO_TREE_ADD;
	if (obs_audio_render_individual(type, output_flags) && !duplicated)
		return OBS_AUDIO_TREE_DUPLICATE;
	return OBS_AUDIO_TREE_NONE;
}

bool obs_audio_render_silence(const struct obs_audio_monitor_dedup *dedup, bool monitoring_enabled,
			      bool is_dedup_source)
{
	if (!dedup->set || !dedup->active)
		return false;

	bool fader_muted = close_float(dedup->volume, 0.0f, 0.0001f);
	bool output_capture_unmuted = !dedup->muted && !fader_muted;

	return output_capture_unmuted && monitoring_enabled && !is_dedup_source;
}

uint32_t obs_audio_render_clear_mixes(uint32_t dedup_mixers, uint32_t source_mixers)
{
	return dedup_mixers & source_mixers & ((1u << MAX_AUDIO_MIXES) - 1);
}

enum obs_audio_backwards_choice obs_audio_render_backwards(bool maxed, uint64_t audio_ts, uint64_t tick_start,
							   bool audio_render)
{
	if (!maxed || audio_ts == 0 || audio_ts >= tick_start)
		return OBS_AUDIO_BACKWARDS_NONE;
	return audio_render ? OBS_AUDIO_BACKWARDS_PENDING : OBS_AUDIO_BACKWARDS_IGNORE;
}

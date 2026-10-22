/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
                          Philippe Groarke <philippe.groarke@gmail.com>

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

#include "libobs/build/libobs-ported-scene-audio-actions.pch.h"

/* The core of the scene item visibility audio ramp: the loop of
 * apply_scene_item_audio_actions() and the window of
 * apply_scene_item_volume(), on the item's fields alone. It reaches no
 * scene item and no source: obs-scene-item-audio-actions.c passes the
 * item's queue and visibility with actions_mutex held, and drops one active
 * reference per hide that obs_scene_audio_actions_apply() returns.
 * Declared in obs-scene-audio-actions.h. Ported to rust/obs-scene
 * (audio_actions): the hybrid libobs drops this file. */

size_t obs_scene_audio_actions_apply(obs_scene_item_action_array_t *actions, bool *visible, uint64_t ts,
				     size_t sample_rate, float *buf)
{
	bool cur_visible = *visible;
	uint64_t frame_num = 0;
	size_t deref_count = 0;

	for (size_t i = 0; i < actions->num; i++) {
		struct item_action action = actions->array[i];
		uint64_t timestamp = action.timestamp;
		uint64_t new_frame_num;

		if (timestamp < ts)
			timestamp = ts;

		new_frame_num = util_mul_div64(timestamp - ts, sample_rate, 1000000000ULL);

		if (ts && new_frame_num >= AUDIO_OUTPUT_FRAMES)
			break;

		da_erase(*actions, i--);

		*visible = action.visible;
		if (!*visible)
			deref_count++;

		if (buf && new_frame_num > frame_num) {
			for (; frame_num < new_frame_num; frame_num++)
				buf[frame_num] = cur_visible ? 1.0f : 0.0f;
		}

		cur_visible = *visible;
	}

	if (buf) {
		for (; frame_num < AUDIO_OUTPUT_FRAMES; frame_num++)
			buf[frame_num] = cur_visible ? 1.0f : 0.0f;
	}

	return deref_count;
}

bool obs_scene_audio_action_due(uint64_t timestamp, uint64_t ts, size_t sample_rate)
{
	uint64_t duration = util_mul_div64(AUDIO_OUTPUT_FRAMES, 1000000000ULL, sample_rate);

	return !ts || timestamp < (ts + duration);
}

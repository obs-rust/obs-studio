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

#include "libobs/build/libobs.pch.h"

/* Scene item visibility audio ramp, moved from obs-scene.c: showing or hiding
 * an item whose source has audio queues an item_action with the time of the
 * call, and these apply the queued actions on the samples they fall on.
 * scene_audio_render() calls apply_scene_item_volume() for each item of a
 * tick, and process_all_audio_actions() when the scene has no audio playing;
 * scene_video_tick() calls apply_scene_item_audio_actions() with no buffer
 * for a scene that is only showing. Declared in obs-internal.h. */

void apply_scene_item_audio_actions(struct obs_scene_item *item, float *buf, uint64_t ts, size_t sample_rate)
{
	bool cur_visible = item->visible;
	uint64_t frame_num = 0;
	size_t deref_count = 0;

	pthread_mutex_lock(&item->actions_mutex);

	for (size_t i = 0; i < item->audio_actions.num; i++) {
		struct item_action action = item->audio_actions.array[i];
		uint64_t timestamp = action.timestamp;
		uint64_t new_frame_num;

		if (timestamp < ts)
			timestamp = ts;

		new_frame_num = util_mul_div64(timestamp - ts, sample_rate, 1000000000ULL);

		if (ts && new_frame_num >= AUDIO_OUTPUT_FRAMES)
			break;

		da_erase(item->audio_actions, i--);

		item->visible = action.visible;
		if (!item->visible)
			deref_count++;

		if (buf && new_frame_num > frame_num) {
			for (; frame_num < new_frame_num; frame_num++)
				buf[frame_num] = cur_visible ? 1.0f : 0.0f;
		}

		cur_visible = item->visible;
	}

	if (buf) {
		for (; frame_num < AUDIO_OUTPUT_FRAMES; frame_num++)
			buf[frame_num] = cur_visible ? 1.0f : 0.0f;
	}

	pthread_mutex_unlock(&item->actions_mutex);

	while (deref_count--) {
		if (os_atomic_dec_long(&item->active_refs) == 0) {
			obs_source_remove_active_child(item->parent->source, item->source);
		}
	}
}

bool apply_scene_item_volume(struct obs_scene_item *item, float *buf, uint64_t ts, size_t sample_rate)
{
	bool actions_pending;
	struct item_action action;

	pthread_mutex_lock(&item->actions_mutex);

	actions_pending = item->audio_actions.num > 0;
	if (actions_pending)
		action = item->audio_actions.array[0];

	pthread_mutex_unlock(&item->actions_mutex);

	if (actions_pending) {
		uint64_t duration = util_mul_div64(AUDIO_OUTPUT_FRAMES, 1000000000ULL, sample_rate);

		if (!ts || action.timestamp < (ts + duration)) {
			apply_scene_item_audio_actions(item, buf, ts, sample_rate);
			return true;
		}
	}

	return false;
}

void process_all_audio_actions(struct obs_scene_item *item, size_t sample_rate)
{
	while (apply_scene_item_volume(item, NULL, 0, sample_rate))
		;
}

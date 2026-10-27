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
 * for a scene that is only showing. Declared in obs-internal.h.
 *
 * This is the item's side: it takes actions_mutex, passes the item's queue
 * and visibility to the core (obs-scene-audio-actions.c), and drops one
 * active reference per hide the core returns, after the lock, removing the
 * active child when the count reaches 0. */

void apply_scene_item_audio_actions(struct obs_scene_item *item, float *buf, uint64_t ts, size_t sample_rate)
{
	size_t deref_count;

	pthread_mutex_lock(&item->actions_mutex);
	deref_count = obs_scene_audio_actions_apply(&item->audio_actions, &item->visible, ts, sample_rate, buf);
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
	/* Set only when an action is pending, and read under the same guard:
	 * C4701 at /W4. Zero-initializing cannot be observed. */
	struct item_action action = {0};

	pthread_mutex_lock(&item->actions_mutex);

	actions_pending = item->audio_actions.num > 0;
	if (actions_pending)
		action = item->audio_actions.array[0];

	pthread_mutex_unlock(&item->actions_mutex);

	if (actions_pending && obs_scene_audio_action_due(action.timestamp, ts, sample_rate)) {
		apply_scene_item_audio_actions(item, buf, ts, sample_rate);
		return true;
	}

	return false;
}

void process_all_audio_actions(struct obs_scene_item *item, size_t sample_rate)
{
	while (apply_scene_item_volume(item, NULL, 0, sample_rate))
		;
}

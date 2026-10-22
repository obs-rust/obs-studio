/*
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
*/

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "util/darray.h"

/* The scene item visibility audio ramp, on the item's fields alone
 * (obs-scene-audio-actions.c). The item's side (actions_mutex, active_refs,
 * obs_source_remove_active_child()) stays in obs-scene-item-audio-actions.c,
 * which passes &item->audio_actions and &item->visible with the lock held.
 * Neither function takes a lock, calls into libobs or keeps state.
 *
 * obs_scene_audio_actions_apply() consumes the queued actions that fall in
 * the audio tick at ts, in queue order, erasing each from `actions`, and
 * stops at the first one on frame AUDIO_OUTPUT_FRAMES or later (frame
 * floor((timestamp - ts) * sample_rate / 1e9), a timestamp before ts on
 * frame 0). With ts 0 it consumes them all, whatever their time. It sets
 * *visible to each action in turn and, when buf is not NULL, fills buf
 * with 1.0 or 0.0, the visibility before each change, up to its frame, and
 * the rest of the tick with the visibility after the last one. It returns
 * the number of hides consumed. buf is AUDIO_OUTPUT_FRAMES gains; with ts 0
 * and a buffer the fill is not bounded by the tick and writes past it, so
 * libobs passes ts 0 only with a NULL buffer (process_all_audio_actions(),
 * the preview-only video tick).
 *
 * obs_scene_audio_action_due() is the window of apply_scene_item_volume():
 * whether an action at `timestamp` is due in the tick at ts, that is ts is
 * 0 or timestamp is before ts plus AUDIO_OUTPUT_FRAMES frames in ns,
 * rounded down. It divides by sample_rate, which must not be 0.
 *
 * Hidden in libobs: not EXPORT. */

struct item_action {
	bool visible;
	uint64_t timestamp;
};

/* Named so that the core can take a pointer to the item's queue. */
typedef DARRAY(struct item_action) obs_scene_item_action_array_t;

size_t obs_scene_audio_actions_apply(obs_scene_item_action_array_t *actions, bool *visible, uint64_t ts,
				     size_t sample_rate, float *buf);
bool obs_scene_audio_action_due(uint64_t timestamp, uint64_t ts, size_t sample_rate);

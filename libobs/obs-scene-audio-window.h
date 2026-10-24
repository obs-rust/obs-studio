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

/* The audio window decisions of scene_audio_render() in obs-scene.c, on
 * plain values (obs-scene-audio-window.c). Each one is made where the C
 * made it, from values the caller reads at that point:
 * - obs_scene_audio_window_pick(): which source an item's audio is read
 *   from, the show transition, the hide transition or the source;
 * - obs_scene_audio_window_counts() and obs_scene_audio_window_earliest():
 *   which items the tick's timestamp is the earliest of, and the fold;
 * - obs_scene_audio_window_silent(): no item counted, so the scene only
 *   processes its items' pending audio actions and reports no audio;
 * - obs_scene_audio_window_frames(): the frames of the tick an item's audio
 *   lands on, or none;
 * - obs_scene_audio_window_mixes(): whether a hidden item is still mixed;
 * - obs_scene_audio_window_adds(): whether the item's mix is added, or
 *   skipped as duplicated audio.
 *
 * obs-scene.c keeps audio_lock, apply_scene_item_volume() and
 * process_all_audio_actions() (it passes the volume result),
 * obs_source_audio_pending(), obs_source_get_audio_timestamp(),
 * obs_source_get_audio_mix() and the two mix loops, and reads every item
 * and source field itself, each one only where the old expressions read
 * it: a value the old short circuit did not read is passed as false or 0,
 * and no decision here looks at it. Nothing here takes a lock, calls into
 * libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* The source scene_audio_render() reads an item's audio from. */
enum obs_scene_audio_window_child {
	/* item->source */
	OBS_SCENE_AUDIO_WINDOW_SOURCE,
	/* item->show_transition */
	OBS_SCENE_AUDIO_WINDOW_SHOW,
	/* item->hide_transition */
	OBS_SCENE_AUDIO_WINDOW_HIDE,
};

/* visible: item->visible. show_active: scene_transition_active() of
 * item->show_transition, asked only when the item is visible. hide_active:
 * the same of item->hide_transition, asked only when the show transition is
 * not picked and the item, read again, is not visible; visible is then that
 * second read. */
enum obs_scene_audio_window_child obs_scene_audio_window_pick(bool visible, bool show_active, bool hide_active);

/* Whether an item's timestamp takes part in the tick's earliest timestamp.
 * pending: obs_source_audio_pending() of the picked source. visible and
 * hide_active: item->visible and scene_transition_active() of
 * item->hide_transition, asked only when the source is not pending, and the
 * hide transition only when the item is not visible. */
bool obs_scene_audio_window_counts(bool pending, bool visible, bool hide_active);

/* The earliest timestamp so far, with an item's source_ts
 * (obs_source_get_audio_timestamp()) folded in: 0 is no timestamp, on
 * either side. */
uint64_t obs_scene_audio_window_earliest(uint64_t timestamp, uint64_t source_ts);

/* The scene has no audio this tick: timestamp, the earliest, is 0. */
bool obs_scene_audio_window_silent(uint64_t timestamp);

/* Where an item's audio lands in the tick. */
struct obs_scene_audio_window {
	/* mix the item: false for a pending source, no timestamp, or a start
	 * on frame AUDIO_OUTPUT_FRAMES or later (pos and count are then 0) */
	bool mix;
	/* the first frame of the tick the item's audio is added at */
	size_t pos;
	/* the frames added, AUDIO_OUTPUT_FRAMES - pos */
	size_t count;
};

/* pending: obs_source_audio_pending() of the picked source. source_ts:
 * obs_source_get_audio_timestamp() of it, asked only when it is not
 * pending. timestamp: the tick's earliest timestamp. pos is
 * ns_to_audio_frames(sample_rate, source_ts - timestamp) in uint64_t,
 * where a source_ts before timestamp wraps to a frame past the tick. */
struct obs_scene_audio_window obs_scene_audio_window_frames(bool pending, uint64_t source_ts, uint64_t timestamp,
							    size_t sample_rate);

/* Whether an item in the window is mixed. apply_buf:
 * apply_scene_item_volume()'s result. visible and hide_active: item->visible
 * and scene_transition_active() of item->hide_transition, asked only without
 * a volume buffer, and the hide transition only when the item is not
 * visible. */
bool obs_scene_audio_window_mixes(bool apply_buf, bool visible, bool hide_active);

/* Whether the item's mix, once read (obs_source_get_audio_mix()), is added
 * to the scene's. audio_is_duplicated: the picked source's flag. */
bool obs_scene_audio_window_adds(bool audio_is_duplicated);

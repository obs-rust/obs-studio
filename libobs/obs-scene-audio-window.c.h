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

#include "libobs/build/libobs-ported-scene-audio-window.pch.h"

/* Ported to rust/obs-scene (audio_window): the hybrid libobs drops this file.
 * The audio window decisions of scene_audio_render() in obs-scene.c, on
 * plain values: the source each item is read from, the tick's earliest
 * timestamp, and the frames each item is mixed on. obs-scene.c keeps the
 * lock, the volume buffer and the audio actions, every libobs call and the
 * mix loops, and passes the values it reads. Declared in
 * obs-scene-audio-window.h without EXPORT. */

enum obs_scene_audio_window_child obs_scene_audio_window_pick(bool visible, bool show_active, bool hide_active)
{
	if (visible && show_active)
		return OBS_SCENE_AUDIO_WINDOW_SHOW;
	else if (!visible && hide_active)
		return OBS_SCENE_AUDIO_WINDOW_HIDE;
	return OBS_SCENE_AUDIO_WINDOW_SOURCE;
}

bool obs_scene_audio_window_counts(bool pending, bool visible, bool hide_active)
{
	return !pending && (visible || hide_active);
}

uint64_t obs_scene_audio_window_earliest(uint64_t timestamp, uint64_t source_ts)
{
	if (source_ts && (!timestamp || source_ts < timestamp))
		timestamp = source_ts;
	return timestamp;
}

bool obs_scene_audio_window_silent(uint64_t timestamp)
{
	return !timestamp;
}

struct obs_scene_audio_window obs_scene_audio_window_frames(bool pending, uint64_t source_ts, uint64_t timestamp,
							    size_t sample_rate)
{
	struct obs_scene_audio_window window = {0};
	size_t pos;

	if (pending)
		return window;

	if (!source_ts)
		return window;

	pos = (size_t)ns_to_audio_frames(sample_rate, source_ts - timestamp);

	if (pos >= AUDIO_OUTPUT_FRAMES)
		return window;

	window.mix = true;
	window.pos = pos;
	window.count = AUDIO_OUTPUT_FRAMES - pos;
	return window;
}

bool obs_scene_audio_window_mixes(bool apply_buf, bool visible, bool hide_active)
{
	return !(!apply_buf && !visible && !hide_active);
}

bool obs_scene_audio_window_adds(bool audio_is_duplicated)
{
	return !audio_is_duplicated;
}

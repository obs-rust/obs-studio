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

#include "libobs/build/libobs-ported-source-media-admission.pch.h"

/* The media action admission decisions of obs-source.c, moved from the
 * obs_source_media_*() functions (the controllable and callback checks and
 * the getters' fallbacks) and process_media_actions() (the callback and
 * signal of each queued action): the decisions, on plain values.
 * obs-source.c keeps data_valid(), every field read, the action queue and
 * its mutex, the callbacks and the signals, and acts on what comes back.
 * Declared in obs-source-media-admission.h without EXPORT. In hybrid libobs
 * the Rust port (rust/obs-source, media_admission) replaces this file. */

enum obs_source_media_admit obs_source_media_admit(bool valid)
{
	if (!valid)
		return OBS_SOURCE_MEDIA_ADMIT_REJECT;

	return OBS_SOURCE_MEDIA_ADMIT_READ_FLAGS;
}

enum obs_source_media_admit obs_source_media_admit_flags(enum obs_source_media_call call, uint32_t output_flags)
{
	if ((output_flags & OBS_SOURCE_MEDIA_CONTROLLABLE) == 0)
		return OBS_SOURCE_MEDIA_ADMIT_REJECT;

	switch (call) {
	case OBS_SOURCE_MEDIA_CALL_STARTED:
	case OBS_SOURCE_MEDIA_CALL_ENDED:
		return OBS_SOURCE_MEDIA_ADMIT_ACCEPT;
	case OBS_SOURCE_MEDIA_CALL_PLAY_PAUSE:
	case OBS_SOURCE_MEDIA_CALL_RESTART:
	case OBS_SOURCE_MEDIA_CALL_STOP:
	case OBS_SOURCE_MEDIA_CALL_NEXT:
	case OBS_SOURCE_MEDIA_CALL_PREVIOUS:
	case OBS_SOURCE_MEDIA_CALL_GET_DURATION:
	case OBS_SOURCE_MEDIA_CALL_GET_TIME:
	case OBS_SOURCE_MEDIA_CALL_SET_TIME:
	case OBS_SOURCE_MEDIA_CALL_GET_STATE:
		break;
	}

	return OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK;
}

enum obs_source_media_admit obs_source_media_admit_callback(bool has_callback)
{
	if (!has_callback)
		return OBS_SOURCE_MEDIA_ADMIT_REJECT;

	return OBS_SOURCE_MEDIA_ADMIT_ACCEPT;
}

int64_t obs_source_media_time_fallback(void)
{
	return 0;
}

int obs_source_media_state_fallback(void)
{
	return OBS_SOURCE_MEDIA_STATE_NONE;
}

enum obs_source_media_dispatch obs_source_media_dispatch(int type)
{
	switch (type) {
	case OBS_SOURCE_MEDIA_ACTION_NONE:
		return OBS_SOURCE_MEDIA_DISPATCH_RETURN;
	case OBS_SOURCE_MEDIA_ACTION_PLAY_PAUSE:
		return OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE;
	case OBS_SOURCE_MEDIA_ACTION_RESTART:
		return OBS_SOURCE_MEDIA_DISPATCH_RESTART;
	case OBS_SOURCE_MEDIA_ACTION_STOP:
		return OBS_SOURCE_MEDIA_DISPATCH_STOP;
	case OBS_SOURCE_MEDIA_ACTION_NEXT:
		return OBS_SOURCE_MEDIA_DISPATCH_NEXT;
	case OBS_SOURCE_MEDIA_ACTION_PREVIOUS:
		return OBS_SOURCE_MEDIA_DISPATCH_PREVIOUS;
	case OBS_SOURCE_MEDIA_ACTION_SET_TIME:
		return OBS_SOURCE_MEDIA_DISPATCH_SET_TIME;
	default:
		return OBS_SOURCE_MEDIA_DISPATCH_SKIP;
	}
}

bool obs_source_media_signal_reads_pause(enum obs_source_media_dispatch dispatch)
{
	return dispatch == OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE;
}

const char *obs_source_media_signal(enum obs_source_media_dispatch dispatch, bool pause)
{
	switch (dispatch) {
	case OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE:
		if (pause)
			return "media_pause";
		else
			return "media_play";
	case OBS_SOURCE_MEDIA_DISPATCH_RESTART:
		return "media_restart";
	case OBS_SOURCE_MEDIA_DISPATCH_STOP:
		return "media_stopped";
	case OBS_SOURCE_MEDIA_DISPATCH_NEXT:
		return "media_next";
	case OBS_SOURCE_MEDIA_DISPATCH_PREVIOUS:
		return "media_previous";
	case OBS_SOURCE_MEDIA_DISPATCH_RETURN:
	case OBS_SOURCE_MEDIA_DISPATCH_SET_TIME:
	case OBS_SOURCE_MEDIA_DISPATCH_SKIP:
		break;
	}

	return NULL;
}

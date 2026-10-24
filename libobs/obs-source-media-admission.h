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

/* The media action admission decisions of obs-source.c, on plain values
 * (obs-source-media-admission.c):
 * - the obs_source_media_*() functions: whether a call is accepted, in the
 *   old order. A source that is not valid (data_valid(), or
 *   obs_source_valid() for _started() and _ended()) is rejected, then one
 *   without OBS_SOURCE_CONTROLLABLE_MEDIA, then, for every call but
 *   _started() and _ended(), one without the call's callback. A READ_ step
 *   asks obs-source.c for the next value, so a field is read only where it
 *   was read before;
 * - the getters' fallbacks: 0 for the duration and the time,
 *   OBS_MEDIA_STATE_NONE for the state;
 * - process_media_actions(): what each queued action does, the callback it
 *   calls and the signal it emits after it.
 *
 * obs-source.c keeps data_valid() and obs_source_valid() (they log), every
 * field read, the action queue and its mutex, the callbacks, the signals
 * and the source refs. None of these functions takes a lock, calls into
 * libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* obs-source.c checks that these are OBS_SOURCE_CONTROLLABLE_MEDIA,
 * OBS_MEDIA_STATE_NONE and enum media_action_type (obs-internal.h). */
#define OBS_SOURCE_MEDIA_CONTROLLABLE (1u << 13)
#define OBS_SOURCE_MEDIA_STATE_NONE 0
#define OBS_SOURCE_MEDIA_ACTION_NONE 0
#define OBS_SOURCE_MEDIA_ACTION_PLAY_PAUSE 1
#define OBS_SOURCE_MEDIA_ACTION_RESTART 2
#define OBS_SOURCE_MEDIA_ACTION_STOP 3
#define OBS_SOURCE_MEDIA_ACTION_NEXT 4
#define OBS_SOURCE_MEDIA_ACTION_PREVIOUS 5
#define OBS_SOURCE_MEDIA_ACTION_SET_TIME 6

/* The public obs_source_media_*() function being admitted. */
enum obs_source_media_call {
	OBS_SOURCE_MEDIA_CALL_PLAY_PAUSE,
	OBS_SOURCE_MEDIA_CALL_RESTART,
	OBS_SOURCE_MEDIA_CALL_STOP,
	OBS_SOURCE_MEDIA_CALL_NEXT,
	OBS_SOURCE_MEDIA_CALL_PREVIOUS,
	OBS_SOURCE_MEDIA_CALL_GET_DURATION,
	OBS_SOURCE_MEDIA_CALL_GET_TIME,
	OBS_SOURCE_MEDIA_CALL_SET_TIME,
	OBS_SOURCE_MEDIA_CALL_GET_STATE,
	OBS_SOURCE_MEDIA_CALL_STARTED,
	OBS_SOURCE_MEDIA_CALL_ENDED,
};

/* An admission answer, or the value to read before it is known. */
enum obs_source_media_admit {
	/* return (a getter returns its fallback) */
	OBS_SOURCE_MEDIA_ADMIT_REJECT,
	/* queue the action, call the getter or emit the signal */
	OBS_SOURCE_MEDIA_ADMIT_ACCEPT,
	/* read source->info.output_flags */
	OBS_SOURCE_MEDIA_ADMIT_READ_FLAGS,
	/* read the call's callback in source->info (!= NULL) */
	OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK,
};

/* What process_media_actions() does with an action it took off the
 * queue. */
enum obs_source_media_dispatch {
	/* MEDIA_ACTION_NONE, the queue was empty: return */
	OBS_SOURCE_MEDIA_DISPATCH_RETURN,
	/* media_play_pause(data, action.pause) */
	OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE,
	/* media_restart(data) */
	OBS_SOURCE_MEDIA_DISPATCH_RESTART,
	/* media_stop(data) */
	OBS_SOURCE_MEDIA_DISPATCH_STOP,
	/* media_next(data) */
	OBS_SOURCE_MEDIA_DISPATCH_NEXT,
	/* media_previous(data) */
	OBS_SOURCE_MEDIA_DISPATCH_PREVIOUS,
	/* media_set_time(data, action.ms) */
	OBS_SOURCE_MEDIA_DISPATCH_SET_TIME,
	/* a type outside the enum: nothing, take the next action */
	OBS_SOURCE_MEDIA_DISPATCH_SKIP,
};

/* --- the obs_source_media_*() functions -------------------------------- */

/* The first step: READ_FLAGS for a valid source, else REJECT. */
enum obs_source_media_admit obs_source_media_admit(bool valid);

/* The step after reading the output flags: REJECT without
 * OBS_SOURCE_CONTROLLABLE_MEDIA, ACCEPT for _started() and _ended(), else
 * READ_CALLBACK (a call outside the enum included). */
enum obs_source_media_admit obs_source_media_admit_flags(enum obs_source_media_call call, uint32_t output_flags);

/* The step after reading the callback: ACCEPT when it is set, else
 * REJECT. */
enum obs_source_media_admit obs_source_media_admit_callback(bool has_callback);

/* What obs_source_media_get_duration() and _get_time() return when the
 * call is rejected: 0. */
int64_t obs_source_media_time_fallback(void);

/* What obs_source_media_get_state() returns when the call is rejected:
 * OBS_MEDIA_STATE_NONE, as int (obs-source.c checks the enum's size). */
int obs_source_media_state_fallback(void);

/* --- process_media_actions() -------------------------------------------- */

/* The dispatch of a queued action's type (enum media_action_type, as
 * int): RETURN for MEDIA_ACTION_NONE, the matching callback for the
 * others, SKIP for a value outside the enum. */
enum obs_source_media_dispatch obs_source_media_dispatch(int type);

/* Whether the signal reads action.pause: only for PLAY_PAUSE (the union
 * holds the time for SET_TIME). */
bool obs_source_media_signal_reads_pause(enum obs_source_media_dispatch dispatch);

/* The signal emitted after the callback: "media_pause" or "media_play"
 * (by pause), "media_restart", "media_stopped", "media_next",
 * "media_previous"; NULL for SET_TIME, RETURN, SKIP and a value outside
 * the enum. pause is false when not read. */
const char *obs_source_media_signal(enum obs_source_media_dispatch dispatch, bool pause);

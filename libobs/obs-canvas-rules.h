/******************************************************************************
    Copyright (C) 2025 by Dennis Sädtler <saedtler@twitch.tv>

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

/* The canvas flag rules and setter checks of obs-canvas.c, on plain values
 * (obs-canvas-rules.c):
 * - the flags each constructor passes: the main canvas's own, and the
 *   public flags of obs_canvas_create(), obs_canvas_create_private() and
 *   obs_load_canvas();
 * - obs_canvas_create_internal() and obs_canvas_reset_video_internal(): the
 *   view type and the mix's mix_audio;
 * - whether a canvas is named, unnamed and signalled globally (a public
 *   canvas), and whether it holds references to its scenes (SCENE_REF);
 * - obs_save_canvas(), obs_canvas_has_valid_video_info(),
 *   obs_canvas_reset_video(), obs_canvas_set_channel(),
 *   obs_canvas_rename_source(), obs_canvas_set_name(), obs_canvas_remove()
 *   and obs_canvas_removed(): their refusals and flag checks.
 *
 * Each decision is its own function, asked at its own point, so
 * obs-canvas.c keeps every read where it was: canvas->flags is read without
 * a lock and obs_canvas_remove() writes it, so each check gets the value its
 * own read gave, and a check behind a || or && is still asked only when the
 * C reached it (obs_source_is_scene() only on a SCENE_REF canvas,
 * obs_video_active() only for a canvas that may reset, strcmp() only for a
 * canvas that may be renamed, each video info field only after the one
 * before it was set). obs-canvas.c also keeps the allocations and frees, the
 * context data calls, the mutexes, the view, the mix and its list, the hash
 * walks (in the #2038 and #2049 order), every reference, the pointer checks
 * (a source, a previous source, a canvas, a mix), the calldata, every signal,
 * the activation calls and the logs. None of these functions takes a lock,
 * calls into libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* enum obs_canvas_flags (obs.h), copied so this header needs no other
 * header; obs-canvas.c checks them. */
#define OBS_CANVAS_RULES_MAIN (1u << 0)
#define OBS_CANVAS_RULES_ACTIVATE (1u << 1)
#define OBS_CANVAS_RULES_MIX_AUDIO (1u << 2)
#define OBS_CANVAS_RULES_SCENE_REF (1u << 3)
#define OBS_CANVAS_RULES_EPHEMERAL (1u << 4)
/* The internal flag that marks a removed canvas. */
#define OBS_CANVAS_RULES_REMOVED (1u << 31)
/* The flags a caller or a scene collection may set: MAIN is the main
 * canvas's own, and REMOVED and unknown bits are dropped. */
#define OBS_CANVAS_RULES_PUBLIC                                                                 \
	(OBS_CANVAS_RULES_ACTIVATE | OBS_CANVAS_RULES_MIX_AUDIO | OBS_CANVAS_RULES_SCENE_REF | \
	 OBS_CANVAS_RULES_EPHEMERAL)
/* VIDEO_FORMAT_NONE (media-io/video-io.h). */
#define OBS_CANVAS_RULES_FORMAT_NONE 0
/* MAX_CHANNELS (obs-defs.h). */
#define OBS_CANVAS_RULES_MAX_CHANNELS 64

/* The flags of the main canvas: MAIN | PROGRAM. */
uint32_t obs_canvas_rules_main_flags(void);

/* The flags kept from a caller or a saved canvas: flags & the public flags.
 * obs_load_canvas() passes the saved int64 cast to uint32_t, as before. */
uint32_t obs_canvas_rules_public_flags(uint32_t flags);

/* Whether the canvas's view is a MAIN_VIEW (else AUX_VIEW): ACTIVATE. */
bool obs_canvas_rules_main_view(uint32_t flags);

/* The mix's mix_audio: MIX_AUDIO. */
bool obs_canvas_rules_mix_audio(uint32_t flags);

/* Whether a canvas is public: named in obs->data.named_canvases, signalled
 * on obs->signals (canvas_create, canvas_rename and canvas_dosignal()'s core
 * signals) and renamed through the hash table. Not for a private one. */
bool obs_canvas_rules_public(bool is_private);

/* Whether the canvas holds a reference to each scene in it: SCENE_REF.
 * obs-canvas.c asks obs_source_is_scene() only when it says so. */
bool obs_canvas_rules_holds_scenes(uint32_t flags);

/* Whether obs_save_canvas() saves the canvas: not EPHEMERAL or REMOVED. */
bool obs_canvas_rules_saves(uint32_t flags);

/* One of obs_canvas_has_valid_video_info()'s size checks: not 0. */
bool obs_canvas_rules_size_set(uint32_t size);

/* obs_canvas_has_valid_video_info()'s format check: not VIDEO_FORMAT_NONE.
 * The format is enum video_format, passed as int. */
bool obs_canvas_rules_format_set(int format);

/* Whether obs_canvas_reset_video() may reset the canvas: not MAIN.
 * obs-canvas.c asks obs_video_active() only when it says so. */
bool obs_canvas_rules_resets(uint32_t flags);

/* Whether obs_canvas_rename_source() and obs_canvas_set_channel() also
 * signal obs->signals (source_rename, channel_change): MAIN. */
bool obs_canvas_rules_global_signals(uint32_t flags);

/* Whether obs_canvas_set_channel() takes the channel: below MAX_CHANNELS. */
bool obs_canvas_rules_channel_ok(uint32_t channel);

/* Whether obs_canvas_set_name() refuses the name: NULL or "". The name's
 * first byte is read only when it is not NULL. */
bool obs_canvas_rules_name_empty(const char *name);

/* Whether obs_canvas_set_name() may rename the canvas: not MAIN.
 * obs-canvas.c compares the names only when it says so. */
bool obs_canvas_rules_renames(uint32_t flags);

/* Whether obs_canvas_remove() removes the canvas: neither REMOVED nor MAIN. */
bool obs_canvas_rules_removable(uint32_t flags);

/* The flags obs_canvas_remove() writes: flags | REMOVED. */
uint32_t obs_canvas_rules_mark_removed(uint32_t flags);

/* obs_canvas_removed(): REMOVED. */
bool obs_canvas_rules_removed(uint32_t flags);

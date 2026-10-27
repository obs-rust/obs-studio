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

#include "libobs/build/libobs-ported-canvas-rules.pch.h"

/* The canvas flag rules and setter checks, moved from obs-canvas.c: the
 * constructors' flags, the view type, mix_audio, the public canvas rule, the
 * scene references, the save, the video info checks and the refusals of
 * obs_canvas_reset_video(), obs_canvas_set_channel(), obs_canvas_set_name()
 * and obs_canvas_remove(), on plain values. obs-canvas.c keeps every read of
 * the canvas's flags at its point, the calls each check guards, the
 * references, the signals and the logs. Declared in obs-canvas-rules.h
 * without EXPORT. Ported to rust/obs-core (canvas_rules): the hybrid libobs
 * drops this file. */

/* obs_create_main_canvas() */
uint32_t obs_canvas_rules_main_flags(void)
{
	return OBS_CANVAS_RULES_MAIN | OBS_CANVAS_RULES_ACTIVATE | OBS_CANVAS_RULES_MIX_AUDIO |
	       OBS_CANVAS_RULES_SCENE_REF;
}

/* obs_canvas_create(), obs_canvas_create_private() and obs_load_canvas():
 * prevent the user from creating a MAIN or removed canvas. */
uint32_t obs_canvas_rules_public_flags(uint32_t flags)
{
	return flags & OBS_CANVAS_RULES_PUBLIC;
}

/* obs_canvas_create_internal() */
bool obs_canvas_rules_main_view(uint32_t flags)
{
	return (flags & OBS_CANVAS_RULES_ACTIVATE) != 0;
}

/* obs_canvas_create_internal() and obs_canvas_reset_video_internal() */
bool obs_canvas_rules_mix_audio(uint32_t flags)
{
	return (flags & OBS_CANVAS_RULES_MIX_AUDIO) != 0;
}

/* canvas_dosignal(), obs_canvas_create_internal(), obs_canvas_destroy() and
 * obs_canvas_set_name() */
bool obs_canvas_rules_public(bool is_private)
{
	return !is_private;
}

/* obs_canvas_destroy(), obs_canvas_insert_source() and
 * obs_canvas_remove_source(): canvases can hold strong refs to scene
 * sources. */
bool obs_canvas_rules_holds_scenes(uint32_t flags)
{
	return (flags & OBS_CANVAS_RULES_SCENE_REF) != 0;
}

/* obs_save_canvas() */
bool obs_canvas_rules_saves(uint32_t flags)
{
	return !(flags & (OBS_CANVAS_RULES_EPHEMERAL | OBS_CANVAS_RULES_REMOVED));
}

/* obs_canvas_has_valid_video_info() */
bool obs_canvas_rules_size_set(uint32_t size)
{
	return size != 0;
}

bool obs_canvas_rules_format_set(int format)
{
	return format != OBS_CANVAS_RULES_FORMAT_NONE;
}

/* obs_canvas_reset_video() */
bool obs_canvas_rules_resets(uint32_t flags)
{
	return !(flags & OBS_CANVAS_RULES_MAIN);
}

/* obs_canvas_rename_source() and obs_canvas_set_channel() */
bool obs_canvas_rules_global_signals(uint32_t flags)
{
	return (flags & OBS_CANVAS_RULES_MAIN) != 0;
}

/* obs_canvas_set_channel() */
bool obs_canvas_rules_channel_ok(uint32_t channel)
{
	return channel < OBS_CANVAS_RULES_MAX_CHANNELS;
}

/* obs_canvas_set_name() */
bool obs_canvas_rules_name_empty(const char *name)
{
	return !name || !*name;
}

/* obs_canvas_set_name(): do not allow renaming main canvases. */
bool obs_canvas_rules_renames(uint32_t flags)
{
	return !(flags & OBS_CANVAS_RULES_MAIN);
}

/* obs_canvas_remove(): do not allow removing the main canvas, or canvases
 * already marked as removed. */
bool obs_canvas_rules_removable(uint32_t flags)
{
	return !(flags & (OBS_CANVAS_RULES_REMOVED | OBS_CANVAS_RULES_MAIN));
}

uint32_t obs_canvas_rules_mark_removed(uint32_t flags)
{
	return flags | OBS_CANVAS_RULES_REMOVED;
}

/* obs_canvas_removed() */
bool obs_canvas_rules_removed(uint32_t flags)
{
	return (flags & OBS_CANVAS_RULES_REMOVED) != 0;
}

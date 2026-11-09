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

#include "libobs/build/libobs.pch.h"

/* Private adapter, not a new public API (obs-display-seam.h). Stubs: each
 * reads, writes and locks nothing until its body is copied from the line of
 * obs-display.c it wraps. */

/* ------------------------------------------------------------------------- */
/* The two mutexes */

void obs_display_seam_init_mutex_values(obs_display_t *display)
{
	(void)display;
}

int obs_display_seam_callbacks_mutex_init(obs_display_t *display)
{
	(void)display;
	return -1;
}

int obs_display_seam_info_mutex_init(obs_display_t *display)
{
	(void)display;
	return -1;
}

void obs_display_seam_callbacks_mutex_destroy(obs_display_t *display)
{
	(void)display;
}

void obs_display_seam_info_mutex_destroy(obs_display_t *display)
{
	(void)display;
}

int obs_display_seam_lock_callbacks(obs_display_t *display)
{
	(void)display;
	return -1;
}

int obs_display_seam_unlock_callbacks(obs_display_t *display)
{
	(void)display;
	return -1;
}

/* ------------------------------------------------------------------------- */
/* The fields */

bool obs_display_seam_get_use_clear_workaround(const obs_display_t *display)
{
	(void)display;
	return false;
}

void obs_display_seam_set_use_clear_workaround(obs_display_t *display, bool value)
{
	(void)display;
	(void)value;
}

gs_swapchain_t *obs_display_seam_get_swap(const obs_display_t *display)
{
	(void)display;
	return NULL;
}

void obs_display_seam_set_swap(obs_display_t *display, gs_swapchain_t *value)
{
	(void)display;
	(void)value;
}

uint32_t obs_display_seam_get_next_cx(const obs_display_t *display)
{
	(void)display;
	return 0;
}

void obs_display_seam_set_next_cx(obs_display_t *display, uint32_t value)
{
	(void)display;
	(void)value;
}

uint32_t obs_display_seam_get_next_cy(const obs_display_t *display)
{
	(void)display;
	return 0;
}

void obs_display_seam_set_next_cy(obs_display_t *display, uint32_t value)
{
	(void)display;
	(void)value;
}

bool obs_display_seam_get_update_color_space(const obs_display_t *display)
{
	(void)display;
	return false;
}

void obs_display_seam_set_update_color_space(obs_display_t *display, bool value)
{
	(void)display;
	(void)value;
}

bool obs_display_seam_get_enabled(const obs_display_t *display)
{
	(void)display;
	return false;
}

void obs_display_seam_set_enabled(obs_display_t *display, bool value)
{
	(void)display;
	(void)value;
}

uint32_t obs_display_seam_get_background_color(const obs_display_t *display)
{
	(void)display;
	return 0;
}

void obs_display_seam_set_background_color(obs_display_t *display, uint32_t value)
{
	(void)display;
	(void)value;
}

/* ------------------------------------------------------------------------- */
/* The draw callbacks */

void obs_display_seam_callbacks_push(obs_display_t *display, const struct draw_callback *callback)
{
	(void)display;
	(void)callback;
}

void obs_display_seam_callbacks_erase_item(obs_display_t *display, const struct draw_callback *callback)
{
	(void)display;
	(void)callback;
}

size_t obs_display_seam_callbacks_num(const obs_display_t *display)
{
	(void)display;
	return 0;
}

struct draw_callback *obs_display_seam_callbacks_get(obs_display_t *display, size_t idx)
{
	(void)display;
	(void)idx;
	return NULL;
}

void obs_display_seam_callbacks_free(obs_display_t *display)
{
	(void)display;
}

/* ------------------------------------------------------------------------- */
/* The display list */

int obs_display_seam_lock_displays(void)
{
	return -1;
}

int obs_display_seam_unlock_displays(void)
{
	return -1;
}

obs_display_t *obs_display_seam_get_first_display(void)
{
	return NULL;
}

void obs_display_seam_set_first_display(obs_display_t *value)
{
	(void)value;
}

obs_display_t **obs_display_seam_first_display_ref(void)
{
	return NULL;
}

obs_display_t *obs_display_seam_get_next(const obs_display_t *display)
{
	(void)display;
	return NULL;
}

void obs_display_seam_set_next(obs_display_t *display, obs_display_t *value)
{
	(void)display;
	(void)value;
}

obs_display_t **obs_display_seam_next_ref(obs_display_t *display)
{
	(void)display;
	return NULL;
}

obs_display_t **obs_display_seam_get_prev_next(const obs_display_t *display)
{
	(void)display;
	return NULL;
}

void obs_display_seam_set_prev_next(obs_display_t *display, obs_display_t **value)
{
	(void)display;
	(void)value;
}

/* ------------------------------------------------------------------------- */
/* The core video */

graphics_t *obs_display_seam_get_graphics(void)
{
	return NULL;
}

gs_effect_t *obs_display_seam_get_solid_effect(void)
{
	return NULL;
}

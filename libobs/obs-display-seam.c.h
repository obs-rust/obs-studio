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

/* Private adapter, not a new public API (obs-display-seam.h). Each body is
 * copied from the line of obs-display.c named beside it. */

/* The clear flags obs-canvas-draw.h copies. */
_Static_assert(OBS_DISPLAY_CLEAR_COLOR == GS_CLEAR_COLOR, "GS_CLEAR_COLOR");
_Static_assert(OBS_DISPLAY_CLEAR_DEPTH == GS_CLEAR_DEPTH, "GS_CLEAR_DEPTH");
_Static_assert(OBS_DISPLAY_CLEAR_STENCIL == GS_CLEAR_STENCIL, "GS_CLEAR_STENCIL");

/* ------------------------------------------------------------------------- */
/* The two mutexes */

/* obs_display_init(): pthread_mutex_init_value() of both */
void obs_display_seam_init_mutex_values(obs_display_t *display)
{
	pthread_mutex_init_value(&display->draw_callbacks_mutex);
	pthread_mutex_init_value(&display->draw_info_mutex);
}

/* obs_display_init(): pthread_mutex_init(&display->draw_callbacks_mutex, NULL) */
int obs_display_seam_callbacks_mutex_init(obs_display_t *display)
{
	return pthread_mutex_init(&display->draw_callbacks_mutex, NULL);
}

/* obs_display_init(): pthread_mutex_init(&display->draw_info_mutex, NULL) */
int obs_display_seam_info_mutex_init(obs_display_t *display)
{
	return pthread_mutex_init(&display->draw_info_mutex, NULL);
}

/* obs_display_free(): pthread_mutex_destroy(&display->draw_callbacks_mutex) */
void obs_display_seam_callbacks_mutex_destroy(obs_display_t *display)
{
	pthread_mutex_destroy(&display->draw_callbacks_mutex);
}

/* obs_display_free(): pthread_mutex_destroy(&display->draw_info_mutex) */
void obs_display_seam_info_mutex_destroy(obs_display_t *display)
{
	pthread_mutex_destroy(&display->draw_info_mutex);
}

/* obs_display_add_draw_callback(), obs_display_remove_draw_callback(),
 * render_display(): pthread_mutex_lock(&display->draw_callbacks_mutex) */
int obs_display_seam_lock_callbacks(obs_display_t *display)
{
	return pthread_mutex_lock(&display->draw_callbacks_mutex);
}

/* The same three: pthread_mutex_unlock(&display->draw_callbacks_mutex) */
int obs_display_seam_unlock_callbacks(obs_display_t *display)
{
	return pthread_mutex_unlock(&display->draw_callbacks_mutex);
}

/* ------------------------------------------------------------------------- */
/* The fields */

/* render_display_begin(): display->use_clear_workaround */
bool obs_display_seam_get_use_clear_workaround(const obs_display_t *display)
{
	return display->use_clear_workaround;
}

/* obs_display_init(): display->use_clear_workaround = .. */
void obs_display_seam_set_use_clear_workaround(obs_display_t *display, bool value)
{
	display->use_clear_workaround = value;
}

/* obs_display_free(), render_display_begin(): display->swap */
gs_swapchain_t *obs_display_seam_get_swap(const obs_display_t *display)
{
	return display->swap;
}

/* obs_display_init(), obs_display_free(): display->swap = .. */
void obs_display_seam_set_swap(obs_display_t *display, gs_swapchain_t *value)
{
	display->swap = value;
}

/* render_display(): display->next_cx */
uint32_t obs_display_seam_get_next_cx(const obs_display_t *display)
{
	return display->next_cx;
}

/* obs_display_init(), obs_display_resize(): display->next_cx = .. */
void obs_display_seam_set_next_cx(obs_display_t *display, uint32_t value)
{
	display->next_cx = value;
}

/* render_display(): display->next_cy */
uint32_t obs_display_seam_get_next_cy(const obs_display_t *display)
{
	return display->next_cy;
}

/* obs_display_init(), obs_display_resize(): display->next_cy = .. */
void obs_display_seam_set_next_cy(obs_display_t *display, uint32_t value)
{
	display->next_cy = value;
}

/* render_display(): display->update_color_space */
bool obs_display_seam_get_update_color_space(const obs_display_t *display)
{
	return display->update_color_space;
}

/* obs_display_update_color_space(), render_display():
 * display->update_color_space = .. */
void obs_display_seam_set_update_color_space(obs_display_t *display, bool value)
{
	display->update_color_space = value;
}

/* render_display(): display->enabled */
bool obs_display_seam_get_enabled(const obs_display_t *display)
{
	return display->enabled;
}

/* obs_display_init(): display->enabled = true */
void obs_display_seam_set_enabled(obs_display_t *display, bool value)
{
	display->enabled = value;
}

/* render_display_begin(): display->background_color */
uint32_t obs_display_seam_get_background_color(const obs_display_t *display)
{
	return display->background_color;
}

/* obs_display_create(): display->background_color = background_color */
void obs_display_seam_set_background_color(obs_display_t *display, uint32_t value)
{
	display->background_color = value;
}

/* ------------------------------------------------------------------------- */
/* The draw callbacks */

/* obs_display_add_draw_callback(): da_push_back(display->draw_callbacks, &data) */
void obs_display_seam_callbacks_push(obs_display_t *display, const struct draw_callback *callback)
{
	da_push_back(display->draw_callbacks, callback);
}

/* obs_display_remove_draw_callback():
 * da_erase_item(display->draw_callbacks, &data) */
void obs_display_seam_callbacks_erase_item(obs_display_t *display, const struct draw_callback *callback)
{
	da_erase_item(display->draw_callbacks, callback);
}

/* render_display(): display->draw_callbacks.num */
size_t obs_display_seam_callbacks_num(const obs_display_t *display)
{
	return display->draw_callbacks.num;
}

/* render_display(): display->draw_callbacks.array + i */
struct draw_callback *obs_display_seam_callbacks_get(obs_display_t *display, size_t idx)
{
	return display->draw_callbacks.array + idx;
}

/* obs_display_free(): da_free(display->draw_callbacks) */
void obs_display_seam_callbacks_free(obs_display_t *display)
{
	da_free(display->draw_callbacks);
}

/* ------------------------------------------------------------------------- */
/* The display list */

/* obs_display_create(), obs_display_destroy():
 * pthread_mutex_lock(&obs->data.displays_mutex) */
int obs_display_seam_lock_displays(void)
{
	return pthread_mutex_lock(&obs->data.displays_mutex);
}

/* The same two: pthread_mutex_unlock(&obs->data.displays_mutex) */
int obs_display_seam_unlock_displays(void)
{
	return pthread_mutex_unlock(&obs->data.displays_mutex);
}

/* obs_display_create(): display->next = obs->data.first_display */
obs_display_t *obs_display_seam_get_first_display(void)
{
	return obs->data.first_display;
}

/* obs_display_create(): obs->data.first_display = display */
void obs_display_seam_set_first_display(obs_display_t *value)
{
	obs->data.first_display = value;
}

/* obs_display_create(): display->prev_next = &obs->data.first_display */
obs_display_t **obs_display_seam_first_display_ref(void)
{
	return &obs->data.first_display;
}

/* obs_display_create(), obs_display_destroy(): display->next */
obs_display_t *obs_display_seam_get_next(const obs_display_t *display)
{
	return display->next;
}

/* obs_display_create(): display->next = .. */
void obs_display_seam_set_next(obs_display_t *display, obs_display_t *value)
{
	display->next = value;
}

/* obs_display_create(): display->next->prev_next = &display->next */
obs_display_t **obs_display_seam_next_ref(obs_display_t *display)
{
	return &display->next;
}

/* obs_display_destroy(): display->prev_next */
obs_display_t **obs_display_seam_get_prev_next(const obs_display_t *display)
{
	return display->prev_next;
}

/* obs_display_create(), obs_display_destroy(): ..->prev_next = .. */
void obs_display_seam_set_prev_next(obs_display_t *display, obs_display_t **value)
{
	display->prev_next = value;
}

/* ------------------------------------------------------------------------- */
/* The core video */

/* obs_display_create(): gs_enter_context(obs->video.graphics) */
graphics_t *obs_display_seam_get_graphics(void)
{
	return obs->video.graphics;
}

/* render_display_begin(): obs->video.solid_effect */
gs_effect_t *obs_display_seam_get_solid_effect(void)
{
	return obs->video.solid_effect;
}

/* ------------------------------------------------------------------------- */
/* The allocation and the graphics data */

/* obs_display_create(): bzalloc(sizeof(struct obs_display)) */
size_t obs_display_seam_size(void)
{
	return sizeof(struct obs_display);
}

/* obs_display_init(): graphics_data->cx */
uint32_t obs_display_seam_get_init_cx(const struct gs_init_data *graphics_data)
{
	return graphics_data->cx;
}

/* obs_display_init(): graphics_data->cy */
uint32_t obs_display_seam_get_init_cy(const struct gs_init_data *graphics_data)
{
	return graphics_data->cy;
}

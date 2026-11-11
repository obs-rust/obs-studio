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

#include "obs-internal.h"

/* Private adapter, not a new public API: the seam between the display
 * (obs-display.c) and the fields it reads and changes in struct obs_display
 * and struct obs_core (obs-display-seam.c).
 *
 * - The two mutexes: obs_display_seam_init_mutex_values(), the
 *   pthread_mutex_init_value() of both; pthread_mutex_init() with no
 *   attributes of draw_callbacks_mutex (_callbacks_mutex_init()) and of
 *   draw_info_mutex (_info_mutex_init()), returning its result, and
 *   pthread_mutex_destroy() of each; lock and unlock of
 *   draw_callbacks_mutex. draw_info_mutex's lock and unlock are
 *   obs_display_size_lock() and _unlock() (obs-display-size-fields.c).
 * - The fields, get and set: use_clear_workaround, swap, next_cx, next_cy,
 *   update_color_space, enabled and background_color. cx and cy are
 *   obs_display_size_fields().
 * - The draw callbacks: da_push_back() and da_erase_item() of one struct
 *   draw_callback, the count, an item (a pointer into the array) and
 *   da_free().
 * - The display list: lock and unlock of obs->data.displays_mutex;
 *   obs->data.first_display, get, set and its address; a display's next,
 *   get, set and its address, and its prev_next (the address of the link
 *   that points at it), get and set.
 * - The core video: obs->video.graphics and obs->video.solid_effect.
 * - The allocation and the graphics data: sizeof(struct obs_display), and
 *   the cx and cy of the struct gs_init_data obs_display_init() is given.
 *
 * Each is a single read, write, lock or container call, copied from the line
 * of obs-display.c it wraps. Each lock and unlock is one pthread_mutex_*()
 * call returning its result. None checks its arguments or takes a lock of
 * its own: a caller takes the same locks around them as obs-display.c does
 * today. That is draw_info_mutex around next_cx, next_cy and
 * update_color_space after init, draw_callbacks_mutex around the draw
 * callbacks after init, and displays_mutex around first_display and the
 * next and prev_next links. The rest belong to the graphics thread, or to
 * init and free, and take no lock.
 *
 * Hidden in libobs: not EXPORT. */

/* The two mutexes. */
void obs_display_seam_init_mutex_values(obs_display_t *display);
int obs_display_seam_callbacks_mutex_init(obs_display_t *display);
int obs_display_seam_info_mutex_init(obs_display_t *display);
void obs_display_seam_callbacks_mutex_destroy(obs_display_t *display);
void obs_display_seam_info_mutex_destroy(obs_display_t *display);
int obs_display_seam_lock_callbacks(obs_display_t *display);
int obs_display_seam_unlock_callbacks(obs_display_t *display);

/* The fields. */
bool obs_display_seam_get_use_clear_workaround(const obs_display_t *display);
void obs_display_seam_set_use_clear_workaround(obs_display_t *display, bool value);
gs_swapchain_t *obs_display_seam_get_swap(const obs_display_t *display);
void obs_display_seam_set_swap(obs_display_t *display, gs_swapchain_t *value);
uint32_t obs_display_seam_get_next_cx(const obs_display_t *display);
void obs_display_seam_set_next_cx(obs_display_t *display, uint32_t value);
uint32_t obs_display_seam_get_next_cy(const obs_display_t *display);
void obs_display_seam_set_next_cy(obs_display_t *display, uint32_t value);
bool obs_display_seam_get_update_color_space(const obs_display_t *display);
void obs_display_seam_set_update_color_space(obs_display_t *display, bool value);
bool obs_display_seam_get_enabled(const obs_display_t *display);
void obs_display_seam_set_enabled(obs_display_t *display, bool value);
uint32_t obs_display_seam_get_background_color(const obs_display_t *display);
void obs_display_seam_set_background_color(obs_display_t *display, uint32_t value);

/* The draw callbacks. */
void obs_display_seam_callbacks_push(obs_display_t *display, const struct draw_callback *callback);
void obs_display_seam_callbacks_erase_item(obs_display_t *display, const struct draw_callback *callback);
size_t obs_display_seam_callbacks_num(const obs_display_t *display);
struct draw_callback *obs_display_seam_callbacks_get(obs_display_t *display, size_t idx);
void obs_display_seam_callbacks_free(obs_display_t *display);

/* The display list. */
int obs_display_seam_lock_displays(void);
int obs_display_seam_unlock_displays(void);
obs_display_t *obs_display_seam_get_first_display(void);
void obs_display_seam_set_first_display(obs_display_t *value);
obs_display_t **obs_display_seam_first_display_ref(void);
obs_display_t *obs_display_seam_get_next(const obs_display_t *display);
void obs_display_seam_set_next(obs_display_t *display, obs_display_t *value);
obs_display_t **obs_display_seam_next_ref(obs_display_t *display);
obs_display_t **obs_display_seam_get_prev_next(const obs_display_t *display);
void obs_display_seam_set_prev_next(obs_display_t *display, obs_display_t **value);

/* The core video. */
graphics_t *obs_display_seam_get_graphics(void);
gs_effect_t *obs_display_seam_get_solid_effect(void);

/* The allocation and the graphics data. */
size_t obs_display_seam_size(void);
uint32_t obs_display_seam_get_init_cx(const struct gs_init_data *graphics_data);
uint32_t obs_display_seam_get_init_cy(const struct gs_init_data *graphics_data);

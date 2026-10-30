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

#include <stddef.h>

/* The volume multiply kernels of obs-source.c, on plain pointers and counts
 * (obs-source-volume-mul.c):
 * - obs_source_volume_mul_block(): multiply_output_audio(), one volume over
 *   a whole mix; apply_audio_volume() passes the mix's first channel buffer
 *   and AUDIO_OUTPUT_FRAMES * channels samples, which works because a mix's
 *   channel buffers are consecutive in allocate_audio_output_buffer()'s one
 *   allocation;
 * - obs_source_volume_mul_data(): multiply_vol_data(), one channel by a
 *   volume per frame; apply_audio_actions() passes each channel of each mix
 *   and its vol_data.
 *
 * obs-source.c keeps the action queue and its mutex, get_source_volume(),
 * the vol_data fill, the memset and the mix and channel loops, in their
 * order. Nothing here allocates, calls into libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* out[i] *= vol for i in [0, count). */
void obs_source_volume_mul_block(float *out, size_t count, float vol);

/* out[i] *= vol_data[i] for i in [0, count). */
void obs_source_volume_mul_data(float *out, const float *vol_data, size_t count);

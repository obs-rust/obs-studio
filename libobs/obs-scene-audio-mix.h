/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
                          Philippe Groarke <philippe.groarke@gmail.com>

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

/* The two mix kernels of scene_audio_render() in obs-scene.c, on plain
 * pointers and counts (obs-scene-audio-mix.c). Each adds one channel of an
 * item's audio into the same channel of one of the scene's mixes:
 * - obs_scene_audio_mix_with_buf(): the item's samples scaled by the 0/1
 *   gain buffer of apply_scene_item_volume(), for an item whose visibility
 *   change falls in the tick;
 * - obs_scene_audio_mix_add(): the item's samples as they are.
 *
 * obs-scene.c keeps the item loop, audio_lock, apply_scene_item_volume(),
 * obs_source_get_audio_mix(), the window and mix decisions, and the mix and
 * channel loops; it passes each channel's buffers here. Nothing here
 * allocates, takes a lock or calls into libobs.
 *
 * Hidden in libobs: not EXPORT. */

/* mix_audio_with_buf(): out[pos + i] += in[i] * buf[i] for i in [0, count).
 * A compiler that contracts (clang's default on aarch64) fuses the multiply
 * and the add; the C oracle of the Rust port is built without contraction. */
void obs_scene_audio_mix_with_buf(float *out, const float *in, const float *buf, size_t pos, size_t count);

/* scene_mix_audio(): out[pos + i] += in[i] for i in [0, count). */
void obs_scene_audio_mix_add(float *out, const float *in, size_t pos, size_t count);

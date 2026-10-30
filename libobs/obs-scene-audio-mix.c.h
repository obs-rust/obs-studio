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

#include "libobs/build/libobs-ported-scene-audio-mix.pch.h"

void obs_scene_audio_mix_with_buf(float *p_out, const float *p_in, const float *buf_in, size_t pos, size_t count)
{
	register float *out = p_out + pos;
	register const float *buf = buf_in;
	register const float *in = p_in;
	register const float *end = in + count;

	while (in < end)
		*(out++) += *(in++) * *(buf++);
}

void obs_scene_audio_mix_add(float *p_out, const float *p_in, size_t pos, size_t count)
{
	register float *out = p_out + pos;
	register const float *in = p_in;
	register const float *end = in + count;

	while (in < end)
		*(out++) += *(in++);
}

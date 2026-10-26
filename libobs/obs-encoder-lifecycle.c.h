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

#include "libobs/build/libobs-ported-encoder-lifecycle.pch.h"

bool obs_encoder_lifecycle_first(size_t num_callbacks)
{
	return num_callbacks == 0;
}

bool obs_encoder_lifecycle_found(size_t idx)
{
	return idx != OBS_ENCODER_LIFECYCLE_NOT_FOUND;
}

bool obs_encoder_lifecycle_last(size_t num_callbacks)
{
	return num_callbacks == 0;
}

bool obs_encoder_lifecycle_pass_texture(uint32_t caps)
{
	return (caps & OBS_ENCODER_LIFECYCLE_CAP_PASS_TEXTURE) != 0;
}

bool obs_encoder_lifecycle_lcm_unset(uint32_t lcm)
{
	return lcm == 0;
}

uint32_t obs_encoder_lifecycle_started_inc(uint32_t started)
{
	return started + 1;
}

bool obs_encoder_lifecycle_group_ready(uint32_t started, size_t num_encoders)
{
	return started >= num_encoders;
}

uint32_t obs_encoder_lifecycle_started_dec(uint32_t started)
{
	return started - 1;
}

bool obs_encoder_lifecycle_group_idle(uint32_t started)
{
	return started == 0;
}

bool obs_encoder_lifecycle_prepend_sei(bool got, bool has_sei, size_t size)
{
	return got && has_sei && size != 0;
}

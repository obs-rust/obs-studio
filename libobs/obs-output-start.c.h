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

#include "libobs/build/libobs-ported-output-start.pch.h"

bool obs_output_start_service(uint32_t flags)
{
	return (flags & OBS_OUTPUT_START_SERVICE) != 0;
}

bool obs_output_start_drop_restart_ref(long delay_restart_refs)
{
	return delay_restart_refs != 0;
}

bool obs_output_start_init_video(uint32_t flags)
{
	return (flags & OBS_OUTPUT_START_VIDEO) != 0;
}

bool obs_output_start_init_audio(uint32_t flags)
{
	return (flags & OBS_OUTPUT_START_AUDIO) != 0;
}

bool obs_output_start_reset_raw(uint32_t flags)
{
	return !((flags & OBS_OUTPUT_START_ENCODED) != 0);
}

bool obs_output_start_pair(uint32_t flags)
{
	return (flags & OBS_OUTPUT_START_VIDEO) != 0 && (flags & OBS_OUTPUT_START_AUDIO) != 0;
}

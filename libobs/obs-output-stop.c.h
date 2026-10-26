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

#include "libobs/build/libobs-ported-output-stop.pch.h"

bool obs_output_stop_encoded(uint32_t flags)
{
	return (flags & OBS_OUTPUT_STOP_ENCODED) != 0;
}

bool obs_output_stop_delay_on(uint64_t active_delay_ns)
{
	return active_delay_ns != 0;
}

bool obs_output_stop_has_video(uint32_t flags)
{
	return (flags & OBS_OUTPUT_STOP_VIDEO) != 0;
}

bool obs_output_stop_has_audio(uint32_t flags)
{
	return (flags & OBS_OUTPUT_STOP_AUDIO) != 0;
}

bool obs_output_stop_service(uint32_t flags)
{
	return (flags & OBS_OUTPUT_STOP_SERVICE) != 0;
}

bool obs_output_stop_end_delay(long delay_restart_refs)
{
	return !delay_restart_refs;
}

bool obs_output_stop_thread_failed(int ret)
{
	return ret != 0;
}

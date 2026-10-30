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

#include "libobs/build/libobs-ported-output-reconnect.pch.h"

uint32_t obs_output_reconnect_first_delay(int retry_sec)
{
	return (uint32_t)(retry_sec * 1000);
}

bool obs_output_reconnect_give_up(int retries, int retry_max)
{
	return retries >= retry_max;
}

bool obs_output_reconnect_grows(int retries)
{
	return retries != 0;
}

uint32_t obs_output_reconnect_next_delay(uint32_t cur_msec, float exp)
{
	uint32_t msec = (uint32_t)((float)cur_msec * exp);
	if (msec > OBS_OUTPUT_RECONNECT_MAX_MSEC) {
		msec = OBS_OUTPUT_RECONNECT_MAX_MSEC;
	}
	return msec;
}

bool obs_output_reconnect_active(int retry_max)
{
	return retry_max != 0;
}

bool obs_output_reconnect_can(bool reconnecting, bool active, int code)
{
	return (reconnecting && code != OBS_OUTPUT_RECONNECT_SUCCESS) ||
	       (active && code == OBS_OUTPUT_RECONNECT_DISCONNECTED);
}

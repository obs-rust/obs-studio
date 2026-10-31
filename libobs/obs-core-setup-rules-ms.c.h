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

#include "libobs/build/libobs-core-setup-rules-ms.pch.h"

/* The audio buffering in milliseconds, moved from obs_reset_audio2() and
 * obs_get_audio_info2() in obs.c: the ticks the reset stored, back in
 * milliseconds at the rate, for the reset's log and the info the getter
 * returns. Pure: no libobs state. obs.c keeps the ticks store, the reads
 * and the logging. Declared in obs-core-setup-rules.h, beside the
 * buffering ticks of obs-core-setup-rules.c. */

/* All int, as written in obs.c: the rate is cast to int and divides, and
 * the product overflows from 2098 ticks. */
int obs_core_setup_rules_buffer_ms(int ticks, uint32_t samples_per_sec)
{
	return ticks * OBS_CORE_SETUP_RULES_AUDIO_OUTPUT_FRAMES * OBS_CORE_SETUP_RULES_SEC_TO_MSEC /
	       (int)samples_per_sec;
}

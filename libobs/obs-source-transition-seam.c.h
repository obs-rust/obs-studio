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

/* Private adapter, not a new public API (obs-source-transition-seam.h). */

bool transition_valid(const obs_source_t *transition, const char *func)
{
	if (!obs_ptr_valid(transition, func))
		return false;
	else if (!obs_source_transition_load_is_transition((int)transition->info.type))
		return false;

	return true;
}

bool transition_active(obs_source_t *transition)
{
	return transition->transitioning_audio || transition->transitioning_video;
}

enum obs_transition_scale_type valid_scale_type(int64_t type)
{
	return (enum obs_transition_scale_type)obs_source_transition_load_scale_type(type);
}

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

/* Private adapter, not a new public API: the seam between the transition
 * code and the transition fields of struct obs_source
 * (obs-source-transition-seam.c).
 *
 * - transition_valid(), transition_active() and valid_scale_type(): the
 *   checks every transition file of libobs.core-source asks, moved from
 *   obs-source-transition.c unchanged.
 *
 * Hidden in libobs: not EXPORT. */

bool transition_valid(const obs_source_t *transition, const char *func);
bool transition_active(obs_source_t *transition);
enum obs_transition_scale_type valid_scale_type(int64_t type);

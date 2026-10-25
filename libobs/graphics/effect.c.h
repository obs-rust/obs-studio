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

#include "build/libobs.pch.h"

void gs_effect_actually_destroy(gs_effect_t *effect)
{
	effect_free(effect);
	bfree(effect);
}

void gs_effect_destroy(gs_effect_t *effect)
{
	if (effect) {
		if (!effect->cached)
			gs_effect_actually_destroy(effect);
	}
}

/* The technique and pass loop with the parameter upload is in
 * effect-passes.c. */

/* The parameter values (gs_effect_set_*, the getters, gs_effect_set_default
 * and gs_effect_set_next_sampler) are in effect-params.c. */

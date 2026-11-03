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

/* Private adapter, not a new public API: the transition sizing helpers that
 * the rest of the transition code calls (obs-source-transition-size.c).
 *
 * - transition_get_cx() and transition_get_cy(): the size a transition
 *   renders at, transition_cx (or _cy) when set, else the actual size.
 * - transition_recalculate_matrices(): the matrix each slot's child is
 *   drawn with, from the child's size, the scale type and the alignment.
 * - transition_recalculate_size(): the actual size, the larger width and
 *   the larger height of the two children.
 *
 * Hidden in libobs: not EXPORT. */

uint32_t transition_get_cx(obs_source_t *tr);
uint32_t transition_get_cy(obs_source_t *tr);
void transition_recalculate_matrices(obs_source_t *transition);
void transition_recalculate_size(obs_source_t *transition);

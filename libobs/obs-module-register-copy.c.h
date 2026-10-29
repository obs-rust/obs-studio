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

#include "libobs/build/libobs-ported-module-register-copy.pch.h"

/* Registration size rules, moved from whole_fields_size(), HAS_VAL(),
 * HANDLE_ERROR(), REGISTER_OBS_DEF() and obs_register_source_s() in
 * obs-module.c: how much of the caller's structure is copied, whether a
 * field lies wholly within the caller's size, the zero and too-large size
 * checks, and the source type that ignores get_width and get_height. Pure:
 * no libobs state. obs-module.c keeps the offsets, the reads of the caller's
 * structure, the copy, the lists, the logging and the error path, and
 * passes the values in. Declared in obs-module-register-copy.h.
 * Ported to rust/obs-core (module_register_copy): the hybrid libobs drops
 * this file. */

/* Every field of the four info structures is a pointer, aligned to and as
 * large as a pointer, except the 32-bit fields each type lists. So the copy
 * ends at the size rounded down to a pointer, or at the end of a 32-bit
 * field that starts there and lies wholly within the size. A size of
 * sizeof() is a multiple of a pointer, so such a copy is whole. */
size_t obs_module_register_copy_whole_size(size_t size, size_t ptr_size, const size_t *u32_fields, size_t count)
{
	size_t whole = size - size % ptr_size;

	for (size_t i = 0; i < count; i++) {
		if (u32_fields[i] >= whole && u32_fields[i] + sizeof(uint32_t) <= size)
			whole = u32_fields[i] + sizeof(uint32_t);
	}
	return whole;
}

bool obs_module_register_copy_within(size_t offset, size_t field_size, size_t size)
{
	return offset + field_size <= size;
}

enum obs_module_register_copy_gate obs_module_register_copy_gate(size_t size, size_t supported)
{
	if (!size)
		return OBS_MODULE_REGISTER_COPY_OUTSIDE_LOAD;

	if (size > supported)
		return OBS_MODULE_REGISTER_COPY_TOO_LARGE;

	return OBS_MODULE_REGISTER_COPY_COPY;
}

bool obs_module_register_copy_ignores_size(uint32_t type)
{
	return type == OBS_MODULE_REGISTER_COPY_SOURCE_TYPE_TRANSITION;
}

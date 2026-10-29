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

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The size rules of type registration in obs-module.c, on plain values
 * (obs-module-register-copy.c). A plugin passes the size of its
 * obs_source_info, obs_output_info, obs_encoder_info or obs_service_info,
 * which is shorter than libobs's when it was built against an older header:
 * - obs_module_register_copy_whole_size(): how much of the caller's
 *   structure is copied: the size rounded down to a pointer, or the end of a
 *   32-bit field that starts there and lies wholly within the size. Every
 *   other field is a pointer, as large as and aligned to ptr_size;
 * - obs_module_register_copy_within(): a field lies wholly within the size,
 *   so it may be read (HAS_VAL() and HANDLE_ERROR());
 * - obs_module_register_copy_gate(): REGISTER_OBS_DEF()'s checks: a size of
 *   0 is a registration outside obs_module_load, a size past what libobs
 *   supports is too large, any other size is copied;
 * - obs_module_register_copy_ignores_size(): a source type whose get_width
 *   and get_height are warned about and ignored (a transition).
 *
 * obs-module.c keeps every offsetof() and sizeof() (passed in, so the rules
 * are right for any pointer size), every read of the caller's structure
 * (each still made only after the in-size answer), the copy, the type lists,
 * the loading module's lists, every log message, and the free_type_data
 * call. Nothing here takes a lock, calls into libobs or keeps state.
 *
 * The source type is copied below, so this header needs no core header;
 * obs-module.c checks it against obs-source.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_SOURCE_TYPE_TRANSITION (enum obs_source_type) */
#define OBS_MODULE_REGISTER_COPY_SOURCE_TYPE_TRANSITION 2

enum obs_module_register_copy_gate {
	OBS_MODULE_REGISTER_COPY_COPY,
	OBS_MODULE_REGISTER_COPY_OUTSIDE_LOAD,
	OBS_MODULE_REGISTER_COPY_TOO_LARGE,
};

size_t obs_module_register_copy_whole_size(size_t size, size_t ptr_size, const size_t *u32_fields, size_t count);
bool obs_module_register_copy_within(size_t offset, size_t field_size, size_t size);
enum obs_module_register_copy_gate obs_module_register_copy_gate(size_t size, size_t supported);
bool obs_module_register_copy_ignores_size(uint32_t type);

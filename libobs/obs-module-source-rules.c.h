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

#include "libobs/build/libobs-ported-module-source-rules.pch.h"

/* Source registration rules, moved from obs_register_source_s() in
 * obs-module.c: the list a source type goes in, the size limit, the output
 * flag rules, the composite and required callback checks, and the versioned
 * id. Pure: no libobs state. obs-module.c keeps the duplicate lookup, the
 * module's source list, the copy, the type lists, the logging and the error
 * path, and passes the type, the flags, the size, which callbacks are inside
 * the size and the version in. Declared in obs-internal.h. Ported to
 * rust/obs-core (module_source_rules): the hybrid libobs drops this file. */

enum obs_module_source_rules_list obs_module_source_rules_list_of(enum obs_source_type type)
{
	if (type == OBS_SOURCE_TYPE_INPUT) {
		return OBS_MODULE_SOURCE_RULES_LIST_INPUT;
	} else if (type == OBS_SOURCE_TYPE_FILTER) {
		return OBS_MODULE_SOURCE_RULES_LIST_FILTER;
	} else if (type == OBS_SOURCE_TYPE_TRANSITION) {
		return OBS_MODULE_SOURCE_RULES_LIST_TRANSITION;
	} else if (type != OBS_SOURCE_TYPE_SCENE) {
		return OBS_MODULE_SOURCE_RULES_LIST_UNKNOWN;
	}

	return OBS_MODULE_SOURCE_RULES_LIST_NONE;
}

bool obs_module_source_rules_size_supported(size_t size, size_t supported)
{
	return !(size > supported);
}

uint32_t obs_module_source_rules_output_flags(enum obs_source_type type, uint32_t output_flags)
{
	/* mark audio-only filters as an async filter categorically */
	if (type == OBS_SOURCE_TYPE_FILTER) {
		if ((output_flags & OBS_SOURCE_VIDEO) == 0)
			output_flags |= OBS_SOURCE_ASYNC;
	}

	if (type == OBS_SOURCE_TYPE_TRANSITION)
		output_flags |= OBS_SOURCE_COMPOSITE | OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW;

	return output_flags;
}

enum obs_module_source_rules_error obs_module_source_rules_check(enum obs_source_type type, uint32_t info_flags,
								 uint32_t flags, uint32_t callbacks)
{
	if ((flags & OBS_SOURCE_COMPOSITE) != 0) {
		if ((flags & OBS_SOURCE_AUDIO) != 0)
			return OBS_MODULE_SOURCE_RULES_COMPOSITE_AUDIO;
		if ((flags & OBS_SOURCE_ASYNC) != 0)
			return OBS_MODULE_SOURCE_RULES_COMPOSITE_ASYNC;
	}

	if ((callbacks & OBS_MODULE_SOURCE_RULES_HAS_GET_NAME) == 0)
		return OBS_MODULE_SOURCE_RULES_NO_GET_NAME;

	if (type != OBS_SOURCE_TYPE_FILTER && type != OBS_SOURCE_TYPE_TRANSITION &&
	    (info_flags & OBS_SOURCE_VIDEO) != 0 && (info_flags & OBS_SOURCE_ASYNC) == 0) {
		if ((callbacks & OBS_MODULE_SOURCE_RULES_HAS_GET_WIDTH) == 0)
			return OBS_MODULE_SOURCE_RULES_NO_GET_WIDTH;
		if ((callbacks & OBS_MODULE_SOURCE_RULES_HAS_GET_HEIGHT) == 0)
			return OBS_MODULE_SOURCE_RULES_NO_GET_HEIGHT;
	}

	if ((flags & OBS_SOURCE_COMPOSITE) != 0) {
		if ((callbacks & OBS_MODULE_SOURCE_RULES_HAS_AUDIO_RENDER) == 0)
			return OBS_MODULE_SOURCE_RULES_NO_AUDIO_RENDER;
	}

	return OBS_MODULE_SOURCE_RULES_OK;
}

char *obs_module_source_rules_versioned_id(const char *id, uint32_t version)
{
	if (version) {
		struct dstr versioned_id = {0};
		dstr_printf(&versioned_id, "%s_v%d", id, (int)version);
		return versioned_id.array;
	}

	return bstrdup(id);
}

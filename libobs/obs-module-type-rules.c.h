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

#include "libobs/build/libobs-ported-module-type-rules.pch.h"

/* Output, encoder and service registration rules, moved from
 * obs_register_output_s(), obs_register_encoder_s() and
 * obs_register_service_s() in obs-module.c: the callbacks each type needs
 * for its flags, capabilities and type, the texture encoder scaling rule, and
 * the protocol dedupe. Pure: no libobs state. obs-module.c keeps the
 * duplicate lookups, the HAS_VAL() size checks, the copy, the type lists, the
 * protocol split and list, the loading module's lists, the logging and the
 * error path, and passes the flags, the capabilities, the type and which
 * callbacks are inside the size in. Declared in obs-module-type-rules.h.
 * Ported to rust/obs-core (module_type_rules): the hybrid libobs drops this
 * file. */

#define HAS(val) ((callbacks & OBS_MODULE_TYPE_RULES_HAS_##val) != 0)

enum obs_module_type_rules_error obs_module_type_rules_output_check(uint32_t flags, uint32_t callbacks)
{
	if (!HAS(GET_NAME))
		return OBS_MODULE_TYPE_RULES_NO_GET_NAME;
	if (!HAS(CREATE))
		return OBS_MODULE_TYPE_RULES_NO_CREATE;
	if (!HAS(DESTROY))
		return OBS_MODULE_TYPE_RULES_NO_DESTROY;
	if (!HAS(START))
		return OBS_MODULE_TYPE_RULES_NO_START;
	if (!HAS(STOP))
		return OBS_MODULE_TYPE_RULES_NO_STOP;

	if (flags & OBS_MODULE_TYPE_RULES_OUTPUT_SERVICE) {
		if (!HAS(PROTOCOLS))
			return OBS_MODULE_TYPE_RULES_NO_PROTOCOLS;
	}

	if (flags & OBS_MODULE_TYPE_RULES_OUTPUT_ENCODED) {
		if (!HAS(ENCODED_PACKET))
			return OBS_MODULE_TYPE_RULES_NO_ENCODED_PACKET;
	} else {
		if (flags & OBS_MODULE_TYPE_RULES_OUTPUT_VIDEO) {
			if (!HAS(RAW_VIDEO))
				return OBS_MODULE_TYPE_RULES_NO_RAW_VIDEO;
		}

		if (flags & OBS_MODULE_TYPE_RULES_OUTPUT_AUDIO) {
			if (flags & OBS_MODULE_TYPE_RULES_OUTPUT_MULTI_TRACK) {
				if (!HAS(RAW_AUDIO2))
					return OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO2;
			} else {
				if (!HAS(RAW_AUDIO))
					return OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO;
			}
		}
	}

	return OBS_MODULE_TYPE_RULES_OK;
}

enum obs_module_type_rules_error obs_module_type_rules_encoder_check(uint32_t type, uint32_t caps, uint32_t callbacks)
{
	if (((caps & OBS_MODULE_TYPE_RULES_CAP_PASS_TEXTURE) != 0 && caps & OBS_MODULE_TYPE_RULES_CAP_SCALING) != 0)
		return OBS_MODULE_TYPE_RULES_TEXTURE_SCALING;

	if (!HAS(GET_NAME))
		return OBS_MODULE_TYPE_RULES_NO_GET_NAME;
	if (!HAS(CREATE))
		return OBS_MODULE_TYPE_RULES_NO_CREATE;
	if (!HAS(DESTROY))
		return OBS_MODULE_TYPE_RULES_NO_DESTROY;

	if ((caps & OBS_MODULE_TYPE_RULES_CAP_PASS_TEXTURE) != 0) {
		if (!HAS(ENCODE_TEXTURE) && !HAS(ENCODE_TEXTURE2))
			return OBS_MODULE_TYPE_RULES_NO_ENCODE_TEXTURE;
	} else {
		if (!HAS(ENCODE))
			return OBS_MODULE_TYPE_RULES_NO_ENCODE;
	}

	if (type == OBS_MODULE_TYPE_RULES_ENCODER_AUDIO) {
		if (!HAS(GET_FRAME_SIZE))
			return OBS_MODULE_TYPE_RULES_NO_GET_FRAME_SIZE;
	}

	return OBS_MODULE_TYPE_RULES_OK;
}

enum obs_module_type_rules_error obs_module_type_rules_service_check(uint32_t callbacks)
{
	if (!HAS(GET_NAME))
		return OBS_MODULE_TYPE_RULES_NO_GET_NAME;
	if (!HAS(CREATE))
		return OBS_MODULE_TYPE_RULES_NO_CREATE;
	if (!HAS(DESTROY))
		return OBS_MODULE_TYPE_RULES_NO_DESTROY;
	if (!HAS(GET_PROTOCOL))
		return OBS_MODULE_TYPE_RULES_NO_GET_PROTOCOL;

	return OBS_MODULE_TYPE_RULES_OK;
}

bool obs_module_type_rules_protocol_listed(const char *protocol, char *const *protocols, size_t num)
{
	bool skip = false;
	for (size_t i = 0; i < num; i++) {
		if (strcmp(protocol, protocols[i]) == 0)
			skip = true;
	}

	return skip;
}

/* File-local macro (BUILD.md): out of the next file in the unity group. */
#undef HAS

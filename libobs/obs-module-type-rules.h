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

/* The output, encoder and service registration rules of obs-module.c, on
 * plain values (obs-module-type-rules.c). Each one is made where the C made
 * it, from values the caller reads at that point:
 * - obs_module_type_rules_output_check(): the first callback an output type
 *   lacks, from its flags: get_name, create, destroy, start and stop always;
 *   protocols for a service output; encoded_packet for an encoded one, else
 *   raw_video for video and raw_audio2 (multi-track) or raw_audio for audio;
 * - obs_module_type_rules_encoder_check(): a texture encoder that scales
 *   itself, then the first callback an encoder type lacks: get_name, create
 *   and destroy; encode_texture or encode_texture2 for a texture encoder,
 *   else encode; get_frame_size for an audio encoder;
 * - obs_module_type_rules_service_check(): the first callback a service
 *   type lacks: get_name, create, destroy and get_protocol;
 * - obs_module_type_rules_protocol_listed(): a protocol of a service output
 *   is already in the list of registered protocols, byte for byte.
 *
 * obs-module.c keeps the duplicate lookups (find_output(), find_encoder(),
 * find_service()), the size-aware HAS_VAL() checks (passed in as the
 * OBS_MODULE_TYPE_RULES_HAS_* bits below), the size limit (through
 * obs_module_source_rules_size_supported()), the copy and the type lists,
 * the protocol split and list, the loading module's lists, every log message
 * (picked from the returned code) and the error path. Nothing here takes a
 * lock, calls into libobs or keeps state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-module.c checks them against obs-output.h and obs-encoder.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_OUTPUT_VIDEO, OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_SERVICE,
 * OBS_OUTPUT_MULTI_TRACK */
#define OBS_MODULE_TYPE_RULES_OUTPUT_VIDEO (1 << 0)
#define OBS_MODULE_TYPE_RULES_OUTPUT_AUDIO (1 << 1)
#define OBS_MODULE_TYPE_RULES_OUTPUT_ENCODED (1 << 2)
#define OBS_MODULE_TYPE_RULES_OUTPUT_SERVICE (1 << 3)
#define OBS_MODULE_TYPE_RULES_OUTPUT_MULTI_TRACK (1 << 4)
/* OBS_ENCODER_AUDIO (enum obs_encoder_type) */
#define OBS_MODULE_TYPE_RULES_ENCODER_AUDIO 0
/* OBS_ENCODER_CAP_PASS_TEXTURE, OBS_ENCODER_CAP_SCALING */
#define OBS_MODULE_TYPE_RULES_CAP_PASS_TEXTURE (1 << 1)
#define OBS_MODULE_TYPE_RULES_CAP_SCALING (1 << 5)

/* A callback HAS_VAL() finds: set, and whole inside the caller's size. */
#define OBS_MODULE_TYPE_RULES_HAS_GET_NAME (1 << 0)
#define OBS_MODULE_TYPE_RULES_HAS_CREATE (1 << 1)
#define OBS_MODULE_TYPE_RULES_HAS_DESTROY (1 << 2)
#define OBS_MODULE_TYPE_RULES_HAS_START (1 << 3)
#define OBS_MODULE_TYPE_RULES_HAS_STOP (1 << 4)
#define OBS_MODULE_TYPE_RULES_HAS_PROTOCOLS (1 << 5)
#define OBS_MODULE_TYPE_RULES_HAS_ENCODED_PACKET (1 << 6)
#define OBS_MODULE_TYPE_RULES_HAS_RAW_VIDEO (1 << 7)
#define OBS_MODULE_TYPE_RULES_HAS_RAW_AUDIO (1 << 8)
#define OBS_MODULE_TYPE_RULES_HAS_RAW_AUDIO2 (1 << 9)
#define OBS_MODULE_TYPE_RULES_HAS_ENCODE (1 << 10)
#define OBS_MODULE_TYPE_RULES_HAS_ENCODE_TEXTURE (1 << 11)
#define OBS_MODULE_TYPE_RULES_HAS_ENCODE_TEXTURE2 (1 << 12)
#define OBS_MODULE_TYPE_RULES_HAS_GET_FRAME_SIZE (1 << 13)
#define OBS_MODULE_TYPE_RULES_HAS_GET_PROTOCOL (1 << 14)

/* The first rule a type breaks. NO_ENCODE_TEXTURE: neither encode_texture
 * nor encode_texture2. */
enum obs_module_type_rules_error {
	OBS_MODULE_TYPE_RULES_OK,
	OBS_MODULE_TYPE_RULES_TEXTURE_SCALING,
	OBS_MODULE_TYPE_RULES_NO_GET_NAME,
	OBS_MODULE_TYPE_RULES_NO_CREATE,
	OBS_MODULE_TYPE_RULES_NO_DESTROY,
	OBS_MODULE_TYPE_RULES_NO_START,
	OBS_MODULE_TYPE_RULES_NO_STOP,
	OBS_MODULE_TYPE_RULES_NO_PROTOCOLS,
	OBS_MODULE_TYPE_RULES_NO_ENCODED_PACKET,
	OBS_MODULE_TYPE_RULES_NO_RAW_VIDEO,
	OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO,
	OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO2,
	OBS_MODULE_TYPE_RULES_NO_ENCODE_TEXTURE,
	OBS_MODULE_TYPE_RULES_NO_ENCODE,
	OBS_MODULE_TYPE_RULES_NO_GET_FRAME_SIZE,
	OBS_MODULE_TYPE_RULES_NO_GET_PROTOCOL,
};

enum obs_module_type_rules_error obs_module_type_rules_output_check(uint32_t flags, uint32_t callbacks);
enum obs_module_type_rules_error obs_module_type_rules_encoder_check(uint32_t type, uint32_t caps, uint32_t callbacks);
enum obs_module_type_rules_error obs_module_type_rules_service_check(uint32_t callbacks);
bool obs_module_type_rules_protocol_listed(const char *protocol, char *const *protocols, size_t num);

/* Test-only oracle: the original libobs/obs-module-register-copy.c.h,
 * unmodified, with its four entry points renamed to oracle_* so they can
 * link next to the Rust shims. The rename also renames the gate enum's tag,
 * so its size is taken from the function's return type. Also the constants
 * of obs-module-register-copy.h and obs-source.h, the offsets of the 32-bit
 * fields obs-module.c lists for each structure, the offsets and sizes of the
 * fields HAS_VAL() and HANDLE_ERROR() read, and the size of each structure,
 * as the C sees them, so the parity tests use the layouts obs-module.c
 * does. */

#include <stddef.h>
#include <stdint.h>

#include "obs.h"

#define obs_module_register_copy_whole_size oracle_obs_module_register_copy_whole_size
#define obs_module_register_copy_within oracle_obs_module_register_copy_within
#define obs_module_register_copy_gate oracle_obs_module_register_copy_gate
#define obs_module_register_copy_ignores_size oracle_obs_module_register_copy_ignores_size

#include "obs-module-register-copy.c.h"

#undef obs_module_register_copy_ignores_size
#undef obs_module_register_copy_gate
#undef obs_module_register_copy_within
#undef obs_module_register_copy_whole_size

uint32_t oracle_module_register_copy_constant(size_t i);
size_t oracle_module_register_copy_u32_fields(uint32_t kind, size_t *out, size_t cap);
size_t oracle_module_register_copy_pointer_fields(uint32_t kind, size_t *out, size_t cap);
size_t oracle_module_register_copy_struct_size(uint32_t kind);

/* In the order of CONSTANT_NAMES (src/module_register_copy.rs). */
static const uint32_t constants[] = {
	OBS_MODULE_REGISTER_COPY_COPY,      OBS_MODULE_REGISTER_COPY_OUTSIDE_LOAD,
	OBS_MODULE_REGISTER_COPY_TOO_LARGE, OBS_MODULE_REGISTER_COPY_SOURCE_TYPE_TRANSITION,
	OBS_SOURCE_TYPE_TRANSITION,         (uint32_t)sizeof(oracle_obs_module_register_copy_gate(0, 0)),
	(uint32_t)sizeof(uint32_t),         (uint32_t)sizeof(void *),
};

uint32_t oracle_module_register_copy_constant(size_t i)
{
	return i < sizeof(constants) / sizeof(constants[0]) ? constants[i] : UINT32_MAX;
}

/* The kinds: 0 source, 1 output, 2 encoder, 3 service. */
#define COPY_OUT(list)                                                    \
	do {                                                              \
		size_t n_ = sizeof(list) / sizeof(list[0]);               \
		for (size_t i_ = 0; i_ < n_ && i_ < cap; i_++)            \
			out[i_] = list[i_];                               \
		return n_;                                                \
	} while (0)

/* The lists of copy_size_obs_*_info() in obs-module.c. */
size_t oracle_module_register_copy_u32_fields(uint32_t kind, size_t *out, size_t cap)
{
	static const size_t source[] = {
		offsetof(struct obs_source_info, type),
		offsetof(struct obs_source_info, output_flags),
		offsetof(struct obs_source_info, icon_type),
		offsetof(struct obs_source_info, version),
	};
	static const size_t output[] = {offsetof(struct obs_output_info, flags)};
	static const size_t encoder[] = {
		offsetof(struct obs_encoder_info, type),
		offsetof(struct obs_encoder_info, caps),
	};

	switch (kind) {
	case 0:
		COPY_OUT(source);
	case 1:
		COPY_OUT(output);
	case 2:
		COPY_OUT(encoder);
	default:
		return 0;
	}
}

/* The fields HAS_VAL() and HANDLE_ERROR() read, offset then size, in pairs:
 * every callback the registrations check, version and caps, type_data and
 * free_type_data. */
#define FIELD(type, f) offsetof(type, f), sizeof(((type *)NULL)->f)

size_t oracle_module_register_copy_pointer_fields(uint32_t kind, size_t *out, size_t cap)
{
	static const size_t source[] = {
		FIELD(struct obs_source_info, get_name),       FIELD(struct obs_source_info, get_width),
		FIELD(struct obs_source_info, get_height),     FIELD(struct obs_source_info, audio_render),
		FIELD(struct obs_source_info, version),        FIELD(struct obs_source_info, type_data),
		FIELD(struct obs_source_info, free_type_data),
	};
	static const size_t output[] = {
		FIELD(struct obs_output_info, get_name),       FIELD(struct obs_output_info, create),
		FIELD(struct obs_output_info, destroy),        FIELD(struct obs_output_info, start),
		FIELD(struct obs_output_info, stop),           FIELD(struct obs_output_info, protocols),
		FIELD(struct obs_output_info, encoded_packet), FIELD(struct obs_output_info, raw_video),
		FIELD(struct obs_output_info, raw_audio),      FIELD(struct obs_output_info, raw_audio2),
		FIELD(struct obs_output_info, type_data),      FIELD(struct obs_output_info, free_type_data),
	};
	static const size_t encoder[] = {
		FIELD(struct obs_encoder_info, get_name),       FIELD(struct obs_encoder_info, create),
		FIELD(struct obs_encoder_info, destroy),        FIELD(struct obs_encoder_info, encode),
		FIELD(struct obs_encoder_info, encode_texture), FIELD(struct obs_encoder_info, encode_texture2),
		FIELD(struct obs_encoder_info, get_frame_size), FIELD(struct obs_encoder_info, caps),
		FIELD(struct obs_encoder_info, type_data),      FIELD(struct obs_encoder_info, free_type_data),
	};
	static const size_t service[] = {
		FIELD(struct obs_service_info, get_name),  FIELD(struct obs_service_info, create),
		FIELD(struct obs_service_info, destroy),   FIELD(struct obs_service_info, get_protocol),
		FIELD(struct obs_service_info, type_data), FIELD(struct obs_service_info, free_type_data),
	};

	switch (kind) {
	case 0:
		COPY_OUT(source);
	case 1:
		COPY_OUT(output);
	case 2:
		COPY_OUT(encoder);
	case 3:
		COPY_OUT(service);
	default:
		return 0;
	}
}

size_t oracle_module_register_copy_struct_size(uint32_t kind)
{
	switch (kind) {
	case 0:
		return sizeof(struct obs_source_info);
	case 1:
		return sizeof(struct obs_output_info);
	case 2:
		return sizeof(struct obs_encoder_info);
	case 3:
		return sizeof(struct obs_service_info);
	default:
		return 0;
	}
}

#undef FIELD
#undef COPY_OUT

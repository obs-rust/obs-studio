/* Test-only oracle: the original libobs/obs-module-type-rules.c.h,
 * unmodified, with its four entry points renamed to oracle_* so they can
 * link next to the Rust shims. Its strcmp() is the C library's. Also the
 * constants of obs-module-type-rules.h, obs-output.h and obs-encoder.h, and
 * the end (offsetof + sizeof) of each field HAS_VAL() reads in obs-module.c
 * and the size of each structure, as the C sees them, so the parity tests
 * find the callbacks inside a size the way obs-module.c does. */

#include <stddef.h>
#include <stdint.h>

#include "obs.h"

#define obs_module_type_rules_output_check oracle_obs_module_type_rules_output_check
#define obs_module_type_rules_encoder_check oracle_obs_module_type_rules_encoder_check
#define obs_module_type_rules_service_check oracle_obs_module_type_rules_service_check
#define obs_module_type_rules_protocol_listed oracle_obs_module_type_rules_protocol_listed

#include "obs-module-type-rules.c.h"

#undef obs_module_type_rules_protocol_listed
#undef obs_module_type_rules_service_check
#undef obs_module_type_rules_encoder_check
#undef obs_module_type_rules_output_check

size_t oracle_module_type_rules_constant_count(void);
uint32_t oracle_module_type_rules_constant(size_t i);
size_t oracle_module_type_rules_field_end(uint32_t kind, uint32_t bit);
size_t oracle_module_type_rules_other_end(uint32_t which);
size_t oracle_module_type_rules_struct_size(uint32_t kind);

/* In the order of CONSTANT_NAMES (src/module_type_rules.rs). */
static const uint32_t constants[] = {
	OBS_MODULE_TYPE_RULES_OUTPUT_VIDEO,
	OBS_MODULE_TYPE_RULES_OUTPUT_AUDIO,
	OBS_MODULE_TYPE_RULES_OUTPUT_ENCODED,
	OBS_MODULE_TYPE_RULES_OUTPUT_SERVICE,
	OBS_MODULE_TYPE_RULES_OUTPUT_MULTI_TRACK,
	OBS_MODULE_TYPE_RULES_ENCODER_AUDIO,
	OBS_MODULE_TYPE_RULES_CAP_PASS_TEXTURE,
	OBS_MODULE_TYPE_RULES_CAP_SCALING,
	OBS_OUTPUT_VIDEO,
	OBS_OUTPUT_AUDIO,
	OBS_OUTPUT_ENCODED,
	OBS_OUTPUT_SERVICE,
	OBS_OUTPUT_MULTI_TRACK,
	OBS_ENCODER_AUDIO,
	OBS_ENCODER_VIDEO,
	OBS_ENCODER_CAP_PASS_TEXTURE,
	OBS_ENCODER_CAP_SCALING,
	OBS_MODULE_TYPE_RULES_HAS_GET_NAME,
	OBS_MODULE_TYPE_RULES_HAS_CREATE,
	OBS_MODULE_TYPE_RULES_HAS_DESTROY,
	OBS_MODULE_TYPE_RULES_HAS_START,
	OBS_MODULE_TYPE_RULES_HAS_STOP,
	OBS_MODULE_TYPE_RULES_HAS_PROTOCOLS,
	OBS_MODULE_TYPE_RULES_HAS_ENCODED_PACKET,
	OBS_MODULE_TYPE_RULES_HAS_RAW_VIDEO,
	OBS_MODULE_TYPE_RULES_HAS_RAW_AUDIO,
	OBS_MODULE_TYPE_RULES_HAS_RAW_AUDIO2,
	OBS_MODULE_TYPE_RULES_HAS_ENCODE,
	OBS_MODULE_TYPE_RULES_HAS_ENCODE_TEXTURE,
	OBS_MODULE_TYPE_RULES_HAS_ENCODE_TEXTURE2,
	OBS_MODULE_TYPE_RULES_HAS_GET_FRAME_SIZE,
	OBS_MODULE_TYPE_RULES_HAS_GET_PROTOCOL,
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
	(uint32_t)sizeof(enum obs_module_type_rules_error),
};

size_t oracle_module_type_rules_constant_count(void)
{
	return sizeof(constants) / sizeof(constants[0]);
}

uint32_t oracle_module_type_rules_constant(size_t i)
{
	return i < sizeof(constants) / sizeof(constants[0]) ? constants[i] : UINT32_MAX;
}

#define END(T, f) (offsetof(struct T, f) + sizeof(((struct T *)0)->f))

/* The end of the field of bit `bit` (the OBS_MODULE_TYPE_RULES_HAS_* bit
 * index) in the structure of `kind` (0 output, 1 encoder, 2 service), or 0
 * when it has none. */
size_t oracle_module_type_rules_field_end(uint32_t kind, uint32_t bit)
{
	switch (kind) {
	case 0: {
		const size_t ends[] = {
			END(obs_output_info, get_name),       END(obs_output_info, create),
			END(obs_output_info, destroy),        END(obs_output_info, start),
			END(obs_output_info, stop),           END(obs_output_info, protocols),
			END(obs_output_info, encoded_packet), END(obs_output_info, raw_video),
			END(obs_output_info, raw_audio),      END(obs_output_info, raw_audio2),
		};
		return bit < sizeof(ends) / sizeof(ends[0]) ? ends[bit] : 0;
	}
	case 1: {
		const size_t ends[] = {
			END(obs_encoder_info, get_name),
			END(obs_encoder_info, create),
			END(obs_encoder_info, destroy),
			0,
			0,
			0,
			0,
			0,
			0,
			0,
			END(obs_encoder_info, encode),
			END(obs_encoder_info, encode_texture),
			END(obs_encoder_info, encode_texture2),
			END(obs_encoder_info, get_frame_size),
		};
		return bit < sizeof(ends) / sizeof(ends[0]) ? ends[bit] : 0;
	}
	case 2: {
		const size_t ends[] = {
			END(obs_service_info, get_name),
			END(obs_service_info, create),
			END(obs_service_info, destroy),
			0,
			0,
			0,
			0,
			0,
			0,
			0,
			0,
			0,
			0,
			0,
			END(obs_service_info, get_protocol),
		};
		return bit < sizeof(ends) / sizeof(ends[0]) ? ends[bit] : 0;
	}
	default:
		return 0;
	}
}

/* Other field ends the registration reads: 0 the output's flags, 1 the
 * encoder's type, 2 its caps; 3, 4, 5 type_data and 6, 7, 8 free_type_data
 * of the output, the encoder and the service. */
size_t oracle_module_type_rules_other_end(uint32_t which)
{
	const size_t ends[] = {
		END(obs_output_info, flags),           END(obs_encoder_info, type),
		END(obs_encoder_info, caps),           END(obs_output_info, type_data),
		END(obs_encoder_info, type_data),      END(obs_service_info, type_data),
		END(obs_output_info, free_type_data),  END(obs_encoder_info, free_type_data),
		END(obs_service_info, free_type_data),
	};
	return which < sizeof(ends) / sizeof(ends[0]) ? ends[which] : 0;
}

size_t oracle_module_type_rules_struct_size(uint32_t kind)
{
	switch (kind) {
	case 0:
		return sizeof(struct obs_output_info);
	case 1:
		return sizeof(struct obs_encoder_info);
	case 2:
		return sizeof(struct obs_service_info);
	default:
		return 0;
	}
}

#undef END

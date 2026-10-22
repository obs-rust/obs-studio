#include "frontend/build/frontend-utility-profile-activate.pch.h"

void profile_activate_config(const struct profile_activate_ops *ops, bool reset)
{
	ops->init_defaults(ops->param);

	/* Defaults live in the loaded configuration only, and a renamed or
	 * duplicated profile is a new copy of basic.ini: set the encoder
	 * defaults on every activation, or keys the profile never saved read
	 * as NULL. */
	ops->init_encoder_defaults(ops->param);

	if (reset) {
		ops->check_encoders(ops->param);
		ops->reset_data(ops->param);
	}
}

void profile_set_encoder_defaults(config_t *config, const char *simple_encoder, const char *aac_encoder)
{
	config_set_default_string(config, "SimpleOutput", "StreamEncoder", simple_encoder);
	config_set_default_string(config, "SimpleOutput", "RecEncoder", simple_encoder);

	config_set_default_string(config, "AdvOut", "AudioEncoder", aac_encoder);
	config_set_default_string(config, "AdvOut", "RecAudioEncoder", aac_encoder);
}

static bool profile_encoder_id_in(const char *id, const char *const *ids)
{
	for (; *ids; ids++) {
		const char *a = id;
		const char *b = *ids;
		while (*a && *a == *b) {
			a++;
			b++;
		}
		if (*a == '\0' && *b == '\0') {
			return true;
		}
	}
	return false;
}

/* The encoders plugins/obs-qsv11 and plugins/obs-ffmpeg's VA-API encoder
 * register, by id. */
static const char *const profile_qsv_encoder_ids[] = {
	"obs_qsv11",         "obs_qsv11_v2",       "obs_qsv11_soft",
	"obs_qsv11_soft_v2", "obs_qsv11_hevc",     "obs_qsv11_hevc_soft",
	"obs_qsv11_av1",     "obs_qsv11_av1_soft", NULL,
};
static const char *const profile_vaapi_encoder_ids[] = {
	"ffmpeg_vaapi",
	"ffmpeg_vaapi_tex",
	"hevc_ffmpeg_vaapi",
	"hevc_ffmpeg_vaapi_tex",
	"av1_ffmpeg_vaapi",
	"av1_ffmpeg_vaapi_tex",
	NULL,
};

enum profile_missing_encoder
profile_missing_encoder_reason(const char *encoder_id, bool (*module_loaded)(const char *name), const char **module)
{
	const char *name = NULL;
	enum profile_missing_encoder no_device = PROFILE_MISSING_ENCODER_UNKNOWN;

	if (encoder_id && profile_encoder_id_in(encoder_id, profile_qsv_encoder_ids)) {
		name = "obs-qsv11";
		no_device = PROFILE_MISSING_ENCODER_NO_QSV_DEVICE;
	} else if (encoder_id && profile_encoder_id_in(encoder_id, profile_vaapi_encoder_ids)) {
		name = "obs-ffmpeg";
		no_device = PROFILE_MISSING_ENCODER_NO_VAAPI_DEVICE;
	}

	if (module) {
		*module = name;
	}
	if (!name) {
		return PROFILE_MISSING_ENCODER_UNKNOWN;
	}
	return module_loaded(name) ? no_device : PROFILE_MISSING_ENCODER_MODULE_NOT_LOADED;
}

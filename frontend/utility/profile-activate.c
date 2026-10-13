#include "profile-activate.h"

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

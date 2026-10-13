#pragma once

#include <stdbool.h>
#include <util/config-file.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Making a profile's configuration the active one, kept free of Qt so it can
 * be unit tested. Defaults (config_set_default_*) live only in the loaded
 * config_t, not in the profile's basic.ini, so they have to be set again on
 * every configuration that becomes active. */

struct profile_activate_ops {
	void *param;

	/* OBSBasic::InitBasicConfigDefaults(): video, audio, output and
	 * hotkey defaults. */
	void (*init_defaults)(void *param);

	/* OBSBasic::InitBasicConfigDefaults2(): the encoder defaults, which
	 * depend on the encoders the loaded modules provide. Calls
	 * profile_set_encoder_defaults(). */
	void (*init_encoder_defaults)(void *param);

	/* OBSBasic::CheckForMissingEncoders() and
	 * OBSBasic::CheckForSimpleModeX264Fallback(). */
	void (*check_encoders)(void *param);

	/* OBSBasic::ResetProfileData(): video, service, outputs, hotkeys. */
	void (*reset_data)(void *param);
};

/* OBSBasic::ActivateProfile() after the profile's basic.ini has been loaded
 * as the active configuration. reset: the profile is a different one, so
 * its settings are applied (switching profiles, a new profile); false when
 * the current profile is renamed or duplicated. The configuration and
 * encoder defaults are set either way. */
void profile_activate_config(const struct profile_activate_ops *ops, bool reset);

/* The encoder defaults of a profile configuration
 * (OBSBasic::InitBasicConfigDefaults2()). simple_encoder: the Simple output
 * mode encoder; aac_encoder: the AAC encoder id for Advanced output mode. */
void profile_set_encoder_defaults(config_t *config, const char *simple_encoder, const char *aac_encoder);

#ifdef __cplusplus
}
#endif

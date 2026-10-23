#pragma once

#include <stdbool.h>
#include <stddef.h>
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

/* Why an encoder the profile uses is not registered, for the Missing
 * Encoders dialog (OBSBasic::CheckForMissingEncoders(),
 * obs-rust/obs-studio#1720). Covers the hardware encoders whose module
 * registers them only when it finds a device that can encode with them:
 * VA-API (obs-ffmpeg) and Intel QSV (obs-qsv11). x264 and NVENC have their
 * own checks in the frontend; any other encoder is UNKNOWN. */
enum profile_missing_encoder {
	PROFILE_MISSING_ENCODER_UNKNOWN,
	/* The module that provides the encoder is not loaded, or not part of
	 * this build. */
	PROFILE_MISSING_ENCODER_MODULE_NOT_LOADED,
	/* obs-ffmpeg is loaded but found no VA-API device that encodes this
	 * format. */
	PROFILE_MISSING_ENCODER_NO_VAAPI_DEVICE,
	/* obs-qsv11 is loaded but found no Intel GPU that encodes this
	 * format. */
	PROFILE_MISSING_ENCODER_NO_QSV_DEVICE,
};

/* module_loaded(name) reports whether the module of that name (for example
 * "obs-qsv11") is loaded. module, when not NULL, receives the module that
 * provides encoder_id, or NULL when the result is UNKNOWN. */
enum profile_missing_encoder
profile_missing_encoder_reason(const char *encoder_id, bool (*module_loaded)(const char *name), const char **module);

/* Why obs-ffmpeg found no VA-API device (PROFILE_MISSING_ENCODER_NO_VAAPI_DEVICE),
 * so the Missing Encoders dialog can name the fix instead of asking the user
 * to debug vainfo (obs-rust/obs-studio#1862). */
enum profile_vaapi_cause {
	/* No /dev/dri/renderD* device: no GPU kernel driver is loaded. */
	PROFILE_VAAPI_NO_RENDER_NODE,
	/* Every render device is an NVIDIA GPU, which has no VA-API encoder. */
	PROFILE_VAAPI_NVIDIA,
	/* The user cannot open the render device (not in the render group). */
	PROFILE_VAAPI_NO_ACCESS,
	/* No VA-API driver file for the GPU is installed. */
	PROFILE_VAAPI_NO_DRIVER,
	/* The driver is installed but encodes no format the profile needs. */
	PROFILE_VAAPI_NO_ENCODE,
};

/* What the diagnosis reads of the host, so tests can fake it. */
struct profile_vaapi_host {
	void *param;
	/* Reads up to size - 1 bytes of the file at path into buf, NUL
	 * terminated. False when it cannot be opened. */
	bool (*read_file)(void *param, const char *path, char *buf, size_t size);
	/* The file at path exists. */
	bool (*exists)(void *param, const char *path);
	/* This user can open the device at path for reading and writing. */
	bool (*can_open_rw)(void *param, const char *path);
};

struct profile_vaapi_diagnosis {
	enum profile_vaapi_cause cause;
	/* The render device diagnosed ("/dev/dri/renderD128"), empty for
	 * PROFILE_VAAPI_NO_RENDER_NODE. */
	char device[32];
	/* Its PCI vendor id, 0 when unknown. */
	unsigned int vendor;
	/* PROFILE_VAAPI_NO_DRIVER: the driver package to install, NULL for a
	 * GPU vendor without a known VA-API driver. */
	const char *package;
	/* PROFILE_VAAPI_NO_DRIVER: the command that installs it on this
	 * distribution, NULL when the distribution is not recognized. */
	const char *install_command;
};

/* The host access the frontend uses: plain stdio on the real paths. */
extern const struct profile_vaapi_host profile_vaapi_system_host;

void profile_vaapi_diagnose(const struct profile_vaapi_host *host, struct profile_vaapi_diagnosis *out);

#ifdef __cplusplus
}
#endif

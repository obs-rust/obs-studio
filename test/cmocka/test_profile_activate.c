#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <util/config-file.h>

#include "profile-activate.h"

/* Upstream obsproject/obs-studio#12458 (reported by usokawa): with a fresh
 * profile, Profile -> Rename (or Duplicate) and then Settings crashes OBS.
 *
 * Rename and Duplicate copy the profile's basic.ini to a new folder and make
 * the copy the active configuration with OBSBasic::ActivateProfile(profile,
 * false). A fresh profile has never stored its encoders: AdvOut/AudioEncoder,
 * AdvOut/RecAudioEncoder and the Simple mode encoders only exist as defaults,
 * which OBSBasic::InitBasicConfigDefaults2() sets on the loaded config_t and
 * which are not written to basic.ini. ActivateProfile() only sets them again
 * when the profile changes, so after a rename they read as NULL, and the
 * Settings dialog crashes in OBSBasicSettings::LoadAdvOutputAudioSettings()
 * on strcmp(NULL, "none").
 *
 * The test loads such a basic.ini into a real config_t, activates it through
 * the helper with callbacks that stand in for OBSBasic, and reads the keys the
 * Settings dialog reads. */

/* What a fresh profile's basic.ini holds after the auto-configuration wizard
 * was cancelled: no encoder keys. */
static const char fresh_profile_ini[] = "[General]\n"
					"Name=Untitled\n"
					"\n"
					"[Video]\n"
					"BaseCX=1920\n"
					"BaseCY=1080\n";

struct frontend_model {
	config_t *config;
	int check_encoders_calls;
	int reset_data_calls;
	struct profile_activate_ops ops;
};

static void model_init_defaults(void *param)
{
	struct frontend_model *m = param;
	/* A few of OBSBasic::InitBasicConfigDefaults()'s defaults. */
	config_set_default_string(m->config, "Output", "Mode", "Simple");
	config_set_default_uint(m->config, "AdvOut", "Track1Bitrate", 160);
}

static void model_init_encoder_defaults(void *param)
{
	struct frontend_model *m = param;
	profile_set_encoder_defaults(m->config, "x264", "ffmpeg_aac");
}

static void model_check_encoders(void *param)
{
	struct frontend_model *m = param;
	m->check_encoders_calls++;
}

static void model_reset_data(void *param)
{
	struct frontend_model *m = param;
	m->reset_data_calls++;
}

static void model_load(struct frontend_model *m, const char *ini)
{
	memset(m, 0, sizeof(*m));
	assert_int_equal(config_open_string(&m->config, ini), CONFIG_SUCCESS);
	m->ops.param = m;
	m->ops.init_defaults = model_init_defaults;
	m->ops.init_encoder_defaults = model_init_encoder_defaults;
	m->ops.check_encoders = model_check_encoders;
	m->ops.reset_data = model_reset_data;
}

static void model_free(struct frontend_model *m)
{
	config_close(m->config);
}

static const char *str_or_null(const char *s)
{
	return s ? s : "(null)";
}

/* The keys OBSBasicSettings::LoadSimpleOutputSettings() and
 * LoadAdvOutputAudioSettings() read and pass to strcmp() and friends. */
static void assert_encoders_readable(config_t *config)
{
	assert_non_null(config_get_string(config, "AdvOut", "AudioEncoder"));
	assert_non_null(config_get_string(config, "AdvOut", "RecAudioEncoder"));
	assert_non_null(config_get_string(config, "SimpleOutput", "StreamEncoder"));
	assert_non_null(config_get_string(config, "SimpleOutput", "RecEncoder"));
}

/* Profile -> Rename or Duplicate on a fresh profile, then Settings. */
static void test_rename_keeps_encoder_defaults(void **state)
{
	(void)state;
	struct frontend_model m;
	model_load(&m, fresh_profile_ini);

	profile_activate_config(&m.ops, false);

	printf("renamed fresh profile: AdvOut/RecAudioEncoder %s, AdvOut/AudioEncoder %s, "
	       "SimpleOutput/StreamEncoder %s\n",
	       str_or_null(config_get_string(m.config, "AdvOut", "RecAudioEncoder")),
	       str_or_null(config_get_string(m.config, "AdvOut", "AudioEncoder")),
	       str_or_null(config_get_string(m.config, "SimpleOutput", "StreamEncoder")));

	assert_encoders_readable(m.config);
	assert_string_equal(config_get_string(m.config, "AdvOut", "RecAudioEncoder"), "ffmpeg_aac");
	model_free(&m);
}

/* Guard: switching to another profile sets the encoder defaults, checks the
 * encoders and resets the profile data, once each. */
static void test_switch_sets_up_everything(void **state)
{
	(void)state;
	struct frontend_model m;
	model_load(&m, fresh_profile_ini);

	profile_activate_config(&m.ops, true);

	assert_encoders_readable(m.config);
	assert_int_equal(m.check_encoders_calls, 1);
	assert_int_equal(m.reset_data_calls, 1);
	model_free(&m);
}

/* Guard: a rename or duplicate keeps running outputs and shows no encoder
 * warnings: no encoder checks, no profile data reset. */
static void test_rename_does_not_reset(void **state)
{
	(void)state;
	struct frontend_model m;
	model_load(&m, fresh_profile_ini);

	profile_activate_config(&m.ops, false);

	assert_int_equal(m.check_encoders_calls, 0);
	assert_int_equal(m.reset_data_calls, 0);
	model_free(&m);
}

/* Guard: encoders the user chose and saved win over the defaults. */
static void test_saved_encoders_win(void **state)
{
	(void)state;
	struct frontend_model m;
	model_load(&m, "[AdvOut]\n"
		       "AudioEncoder=libfdk_aac\n"
		       "RecAudioEncoder=none\n"
		       "[SimpleOutput]\n"
		       "StreamEncoder=nvenc\n"
		       "RecEncoder=x264_lowcpu\n");

	profile_activate_config(&m.ops, false);
	profile_activate_config(&m.ops, true);

	assert_string_equal(config_get_string(m.config, "AdvOut", "AudioEncoder"), "libfdk_aac");
	assert_string_equal(config_get_string(m.config, "AdvOut", "RecAudioEncoder"), "none");
	assert_string_equal(config_get_string(m.config, "SimpleOutput", "StreamEncoder"), "nvenc");
	assert_string_equal(config_get_string(m.config, "SimpleOutput", "RecEncoder"), "x264_lowcpu");
	model_free(&m);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_rename_keeps_encoder_defaults),
		cmocka_unit_test(test_switch_sets_up_everything),
		cmocka_unit_test(test_rename_does_not_reset),
		cmocka_unit_test(test_saved_encoders_win),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}

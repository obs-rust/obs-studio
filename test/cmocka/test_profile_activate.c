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

/* obs-rust/obs-studio#1720: a profile imported into the AppImage used
 * ffmpeg_vaapi_tex and obs_qsv11_v2, and the Missing Encoders dialog said
 * "Unknown (code: unknown)" for both. The AppImage does not ship obs-qsv11,
 * and obs-ffmpeg registers the VA-API encoders only when it finds a VA-API
 * device that encodes the format, so each has a reason the dialog can name. */

/* The modules the AppImage loads: obs-ffmpeg, no obs-qsv11. */
static bool appimage_module_loaded(const char *name)
{
	return strcmp(name, "obs-ffmpeg") == 0;
}

/* A build that has both modules loaded. */
static bool all_modules_loaded(const char *name)
{
	(void)name;
	return true;
}

static bool no_modules_loaded(const char *name)
{
	(void)name;
	return false;
}

static void test_missing_qsv_names_the_module(void **state)
{
	(void)state;
	static const char *const qsv_ids[] = {
		"obs_qsv11",      "obs_qsv11_v2",        "obs_qsv11_soft", "obs_qsv11_soft_v2",
		"obs_qsv11_hevc", "obs_qsv11_hevc_soft", "obs_qsv11_av1",  "obs_qsv11_av1_soft",
	};
	for (size_t i = 0; i < sizeof(qsv_ids) / sizeof(qsv_ids[0]); i++) {
		const char *module = "unset";
		printf("%s without obs-qsv11: reason %d\n", qsv_ids[i],
		       profile_missing_encoder_reason(qsv_ids[i], appimage_module_loaded, &module));
		assert_int_equal(profile_missing_encoder_reason(qsv_ids[i], appimage_module_loaded, &module),
				 PROFILE_MISSING_ENCODER_MODULE_NOT_LOADED);
		assert_non_null(module);
		assert_string_equal(module, "obs-qsv11");

		assert_int_equal(profile_missing_encoder_reason(qsv_ids[i], all_modules_loaded, &module),
				 PROFILE_MISSING_ENCODER_NO_QSV_DEVICE);
		assert_string_equal(module, "obs-qsv11");
	}
}

static void test_missing_vaapi_names_the_device(void **state)
{
	(void)state;
	static const char *const vaapi_ids[] = {
		"ffmpeg_vaapi",          "ffmpeg_vaapi_tex", "hevc_ffmpeg_vaapi",
		"hevc_ffmpeg_vaapi_tex", "av1_ffmpeg_vaapi", "av1_ffmpeg_vaapi_tex",
	};
	for (size_t i = 0; i < sizeof(vaapi_ids) / sizeof(vaapi_ids[0]); i++) {
		const char *module = "unset";
		printf("%s with obs-ffmpeg loaded: reason %d\n", vaapi_ids[i],
		       profile_missing_encoder_reason(vaapi_ids[i], appimage_module_loaded, &module));
		assert_int_equal(profile_missing_encoder_reason(vaapi_ids[i], appimage_module_loaded, &module),
				 PROFILE_MISSING_ENCODER_NO_VAAPI_DEVICE);
		assert_non_null(module);
		assert_string_equal(module, "obs-ffmpeg");

		assert_int_equal(profile_missing_encoder_reason(vaapi_ids[i], no_modules_loaded, &module),
				 PROFILE_MISSING_ENCODER_MODULE_NOT_LOADED);
		assert_string_equal(module, "obs-ffmpeg");
	}
}

/* Guard: other encoders keep the generic "Unknown" (x264 and NVENC have
 * their own checks), and a near miss of an id is not taken for one. */
static void test_other_encoders_stay_unknown(void **state)
{
	(void)state;
	static const char *const other_ids[] = {
		"obs_x264",
		"obs_nvenc_h264_tex",
		"h264_texture_amf",
		"ffmpeg_svt_av1",
		"obs_qsv11_v3",
		"ffmpeg_vaapi_texture",
		"vaapi",
		"",
	};
	for (size_t i = 0; i < sizeof(other_ids) / sizeof(other_ids[0]); i++) {
		const char *module = "unset";
		assert_int_equal(profile_missing_encoder_reason(other_ids[i], all_modules_loaded, &module),
				 PROFILE_MISSING_ENCODER_UNKNOWN);
		assert_null(module);
	}
	assert_int_equal(profile_missing_encoder_reason(NULL, all_modules_loaded, NULL),
			 PROFILE_MISSING_ENCODER_UNKNOWN);
}

/* obs-rust/obs-studio#1862: the host a VA-API diagnosis reads, faked. Each
 * case is one of the causes that took a round of support each in #1720. */
struct fake_file {
	const char *path;
	const char *text;
};

struct fake_host {
	const struct fake_file *files; /* NULL-path terminated; text NULL: not readable */
	const char *const *rw;         /* devices the user can open, NULL terminated */
};

static const struct fake_file *fake_find(const struct fake_host *h, const char *path)
{
	for (const struct fake_file *f = h->files; f && f->path; f++) {
		if (strcmp(f->path, path) == 0) {
			return f;
		}
	}
	return NULL;
}

static bool fake_read_file(void *param, const char *path, char *buf, size_t size)
{
	const struct fake_file *f = fake_find(param, path);
	if (!f || !f->text) {
		return false;
	}
	snprintf(buf, size, "%s", f->text);
	return true;
}

static bool fake_exists(void *param, const char *path)
{
	return fake_find(param, path) != NULL;
}

static bool fake_can_open_rw(void *param, const char *path)
{
	const struct fake_host *h = param;
	for (const char *const *p = h->rw; p && *p; p++) {
		if (strcmp(*p, path) == 0) {
			return true;
		}
	}
	return false;
}

static void diagnose(struct fake_host *h, struct profile_vaapi_diagnosis *out)
{
	const struct profile_vaapi_host host = {h, fake_read_file, fake_exists, fake_can_open_rw};
	memset(out, 0x5a, sizeof(*out));
	profile_vaapi_diagnose(&host, out);
}

static const char ubuntu[] = "PRETTY_NAME=\"Ubuntu 24.04 LTS\"\nID=ubuntu\nID_LIKE=debian\n";
static const char *const rw_128[] = {"/dev/dri/renderD128", NULL};

static void test_vaapi_no_render_node(void **state)
{
	(void)state;
	const struct fake_file files[] = {{"/etc/os-release", ubuntu}, {NULL, NULL}};
	struct fake_host h = {files, NULL};
	struct profile_vaapi_diagnosis d;
	diagnose(&h, &d);
	assert_int_equal(d.cause, PROFILE_VAAPI_NO_RENDER_NODE);
	assert_string_equal(d.device, "");
}

static void test_vaapi_nvidia_only(void **state)
{
	(void)state;
	const struct fake_file files[] = {
		{"/dev/dri/renderD128", ""},
		{"/sys/class/drm/renderD128/device/vendor", "0x10de\n"},
		{NULL, NULL},
	};
	struct fake_host h = {files, rw_128};
	struct profile_vaapi_diagnosis d;
	diagnose(&h, &d);
	assert_int_equal(d.cause, PROFILE_VAAPI_NVIDIA);
	assert_int_equal(d.vendor, 0x10de);
}

/* #1720: the user was not in the render group. A second, NVIDIA, device is
 * skipped in favor of the Intel one. */
static void test_vaapi_no_access(void **state)
{
	(void)state;
	const struct fake_file files[] = {
		{"/dev/dri/renderD128", ""},
		{"/sys/class/drm/renderD128/device/vendor", "0x10de\n"},
		{"/dev/dri/renderD129", ""},
		{"/sys/class/drm/renderD129/device/vendor", "0x8086\n"},
		{"/usr/lib/x86_64-linux-gnu/dri/iHD_drv_video.so", ""},
		{NULL, NULL},
	};
	struct fake_host h = {files, rw_128};
	struct profile_vaapi_diagnosis d;
	diagnose(&h, &d);
	assert_int_equal(d.cause, PROFILE_VAAPI_NO_ACCESS);
	assert_string_equal(d.device, "/dev/dri/renderD129");
	assert_int_equal(d.vendor, 0x8086);
}

/* #1720: an Intel HD 530 on Ubuntu with no Intel VA-API driver installed. */
static void test_vaapi_intel_no_driver_ubuntu(void **state)
{
	(void)state;
	const struct fake_file files[] = {
		{"/etc/os-release", ubuntu},
		{"/dev/dri/renderD128", ""},
		{"/sys/class/drm/renderD128/device/vendor", "0x8086\n"},
		{NULL, NULL},
	};
	struct fake_host h = {files, rw_128};
	struct profile_vaapi_diagnosis d;
	diagnose(&h, &d);
	assert_int_equal(d.cause, PROFILE_VAAPI_NO_DRIVER);
	assert_string_equal(d.device, "/dev/dri/renderD128");
	assert_string_equal(d.package, "intel-media-va-driver-non-free");
	assert_string_equal(d.install_command, "sudo apt install intel-media-va-driver-non-free");
}

static void test_vaapi_amd_no_driver_per_distro(void **state)
{
	(void)state;
	static const struct {
		const char *os_release;
		const char *package;
		const char *command;
	} cases[] = {
		{"ID=debian\n", "mesa-va-drivers", "sudo apt install mesa-va-drivers"},
		{"ID=\"fedora\"\n", "mesa-va-drivers-freeworld", "sudo dnf install mesa-va-drivers-freeworld"},
		{"ID=endeavouros\nID_LIKE=arch\n", "mesa", "sudo pacman -S mesa"},
		{"ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n", "mesa-va-drivers", "sudo apt install mesa-va-drivers"},
		{"ID=gentoo\n", "mesa-va-drivers", NULL},
		{NULL, "mesa-va-drivers", NULL},
	};
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		const struct fake_file files[] = {
			{"/etc/os-release", cases[i].os_release},
			{"/dev/dri/renderD128", ""},
			{"/sys/class/drm/renderD128/device/vendor", "0x1002"},
			{NULL, NULL},
		};
		struct fake_host h = {files, rw_128};
		struct profile_vaapi_diagnosis d;
		diagnose(&h, &d);
		assert_int_equal(d.cause, PROFILE_VAAPI_NO_DRIVER);
		assert_string_equal(d.package, cases[i].package);
		if (cases[i].command) {
			assert_string_equal(d.install_command, cases[i].command);
		} else {
			assert_null(d.install_command);
		}
	}
}

/* Any installed driver for the vendor, in any of the dri directories, counts:
 * the diagnosis then falls back to "cannot encode this format". */
static void test_vaapi_driver_present(void **state)
{
	(void)state;
	static const char *const drivers[] = {
		"/usr/lib/x86_64-linux-gnu/dri/iHD_drv_video.so",
		"/usr/lib/x86_64-linux-gnu/dri/i965_drv_video.so",
		"/usr/lib64/dri/iHD_drv_video.so",
		"/usr/lib/dri/i965_drv_video.so",
		"/run/opengl-driver/lib/dri/iHD_drv_video.so",
	};
	for (size_t i = 0; i < sizeof(drivers) / sizeof(drivers[0]); i++) {
		const struct fake_file files[] = {
			{"/dev/dri/renderD128", ""},
			{"/sys/class/drm/renderD128/device/vendor", "0x8086\n"},
			{drivers[i], ""},
			{NULL, NULL},
		};
		struct fake_host h = {files, rw_128};
		struct profile_vaapi_diagnosis d;
		diagnose(&h, &d);
		assert_int_equal(d.cause, PROFILE_VAAPI_NO_ENCODE);
		assert_null(d.package);
		assert_null(d.install_command);
	}
}

/* A vendor with no known VA-API driver, or an unreadable vendor file: no
 * package to suggest, so the diagnosis is "cannot encode". */
static void test_vaapi_unknown_vendor(void **state)
{
	(void)state;
	const struct fake_file files[] = {
		{"/dev/dri/renderD130", ""},
		{"/sys/class/drm/renderD130/device/vendor", NULL},
		{NULL, NULL},
	};
	static const char *const rw_130[] = {"/dev/dri/renderD130", NULL};
	struct fake_host h = {files, rw_130};
	struct profile_vaapi_diagnosis d;
	diagnose(&h, &d);
	assert_int_equal(d.cause, PROFILE_VAAPI_NO_ENCODE);
	assert_string_equal(d.device, "/dev/dri/renderD130");
	assert_int_equal(d.vendor, 0);
	assert_null(d.package);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_rename_keeps_encoder_defaults),
		cmocka_unit_test(test_switch_sets_up_everything),
		cmocka_unit_test(test_rename_does_not_reset),
		cmocka_unit_test(test_saved_encoders_win),
		cmocka_unit_test(test_missing_qsv_names_the_module),
		cmocka_unit_test(test_missing_vaapi_names_the_device),
		cmocka_unit_test(test_other_encoders_stay_unknown),
		cmocka_unit_test(test_vaapi_no_render_node),
		cmocka_unit_test(test_vaapi_nvidia_only),
		cmocka_unit_test(test_vaapi_no_access),
		cmocka_unit_test(test_vaapi_intel_no_driver_ubuntu),
		cmocka_unit_test(test_vaapi_amd_no_driver_per_distro),
		cmocka_unit_test(test_vaapi_driver_present),
		cmocka_unit_test(test_vaapi_unknown_vendor),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}

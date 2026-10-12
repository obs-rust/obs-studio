/* Regression test for obsproject/obs-studio#4125 (reported by Aktanusa,
 * confirmed by naanlizard; kkartaltepe noted that it stops CQP streams on
 * Linux Intel encoders without CBR): in Advanced output mode, "Ignore
 * streaming service setting recommendations" only keeps the user's bitrate.
 * The service still forces CBR rate control, a VBV buffer size, its B-frame
 * count, its profile and its x264 options on the encoder, so a CQP or CRF
 * stream silently becomes CBR, or does not start on an encoder that has no
 * CBR.
 *
 * service_encoder_settings_apply_video() is the Qt-free step the frontend
 * runs. The first cases give it the recommendations through a stand-in that
 * does what rtmp-services does with them; they need no display, so they run
 * everywhere. The module_ cases load the real rtmp-services module with a
 * services.json written by the test and go through
 * obs_service_apply_encoder_settings(). They need obs_startup(), which needs
 * a display on Linux, so they skip without one (run them under xvfb-run). */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <obs-module.h>
#include <util/dstr.h>
#include <util/platform.h>

#include "service-encoder-settings.h"

#ifdef RTMP_SERVICES_MODULE_PATH
#include "rtmp-format-ver.h"
#endif

/* The recommendations of the test service. */
#define REC_KEYINT 2
#define REC_PROFILE "main"
#define REC_MAX_BITRATE 6000
#define REC_BFRAMES 0
#define REC_X264OPTS "scenecut=0"

static bool module_up;
static obs_service_t *service;
static obs_service_t *custom_service;

/* What rtmp-services' apply_video_encoder_settings() does with the
 * recommendations above. */
static void stand_in_apply(void *param, obs_data_t *settings)
{
	(void)param;

	obs_data_set_int(settings, "keyint_sec", REC_KEYINT);
	obs_data_set_string(settings, "rate_control", "CBR");

	obs_data_item_t *item = obs_data_item_byname(settings, "profile");
	if (obs_data_item_gettype(item) == OBS_DATA_STRING)
		obs_data_set_string(settings, "profile", REC_PROFILE);
	obs_data_item_release(&item);

	if (obs_data_get_int(settings, "bitrate") > REC_MAX_BITRATE) {
		obs_data_set_int(settings, "bitrate", REC_MAX_BITRATE);
		obs_data_set_int(settings, "buffer_size", REC_MAX_BITRATE);
	}

	obs_data_set_int(settings, "bf", REC_BFRAMES);

	struct dstr opts;
	dstr_init_copy(&opts, obs_data_get_string(settings, "x264opts"));
	if (!dstr_is_empty(&opts))
		dstr_cat(&opts, " ");
	dstr_cat(&opts, REC_X264OPTS);
	obs_data_set_string(settings, "x264opts", opts.array);
	dstr_free(&opts);
}

static void apply(bool real_module, obs_data_t *settings, bool ignore_recommended)
{
	if (real_module)
		service_encoder_settings_apply_video(service, settings, ignore_recommended);
	else
		service_encoder_settings_apply_video_with(settings, "rtmp_common", ignore_recommended, stand_in_apply,
							  NULL);
}

/* What the WHIP service does: settings the protocol needs, not
 * recommendations, and no "Ignore" for them. */
static void stand_in_whip_apply(void *param, obs_data_t *settings)
{
	(void)param;
	obs_data_set_int(settings, "bf", 0);
	obs_data_set_bool(settings, "repeat_headers", true);
}

static int failures;

static void expect_str(const char *what, obs_data_t *settings, const char *name, const char *want)
{
	const char *got = obs_data_get_string(settings, name);
	if (strcmp(got, want) != 0) {
		fprintf(stderr, "%s: %s is \"%s\", expected \"%s\"\n", what, name, got, want);
		failures++;
	}
}

static void expect_int(const char *what, obs_data_t *settings, const char *name, long long want)
{
	long long got = obs_data_get_int(settings, name);
	if (got != want) {
		fprintf(stderr, "%s: %s is %lld, expected %lld\n", what, name, got, want);
		failures++;
	}
}

static void expect_unset(const char *what, obs_data_t *settings, const char *name)
{
	if (obs_data_has_user_value(settings, name)) {
		fprintf(stderr, "%s: %s was set\n", what, name);
		failures++;
	}
}

static void check(void)
{
	int n = failures;
	failures = 0;
	if (n)
		fail();
}

/* VAAPI or QSV in CQP, as in kkartaltepe's report. */
static obs_data_t *cqp_settings(void)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "rate_control", "CQP");
	obs_data_set_int(settings, "qp", 20);
	obs_data_set_int(settings, "bitrate", 8000);
	obs_data_set_int(settings, "keyint_sec", 0);
	obs_data_set_int(settings, "bf", 2);
	obs_data_set_string(settings, "profile", "high");
	return settings;
}

/* x264 in CRF, as in the original report. */
static obs_data_t *crf_settings(void)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "rate_control", "CRF");
	obs_data_set_int(settings, "crf", 23);
	obs_data_set_int(settings, "bitrate", 2500);
	obs_data_set_int(settings, "keyint_sec", 2);
	obs_data_set_string(settings, "profile", "high");
	obs_data_set_string(settings, "x264opts", "");
	return settings;
}

static void ignore_keeps_cqp(bool real_module)
{
	const char *what = "ignore, CQP";
	obs_data_t *settings = cqp_settings();

	apply(real_module, settings, true);

	expect_str(what, settings, "rate_control", "CQP");
	expect_int(what, settings, "qp", 20);
	expect_int(what, settings, "bitrate", 8000);
	expect_unset(what, settings, "buffer_size");
	expect_int(what, settings, "bf", 2);
	expect_str(what, settings, "profile", "high");
	expect_unset(what, settings, "x264opts");
	/* An automatic keyframe interval still gets the service's. */
	expect_int(what, settings, "keyint_sec", REC_KEYINT);

	obs_data_release(settings);
	check();
}

static void ignore_keeps_crf(bool real_module)
{
	const char *what = "ignore, CRF";
	obs_data_t *settings = crf_settings();

	apply(real_module, settings, true);

	expect_str(what, settings, "rate_control", "CRF");
	expect_int(what, settings, "crf", 23);
	expect_int(what, settings, "bitrate", 2500);
	expect_str(what, settings, "profile", "high");
	expect_str(what, settings, "x264opts", "");
	expect_int(what, settings, "keyint_sec", 2);

	obs_data_release(settings);
	check();
}

/* Without "Ignore", the service's recommendations are enforced as before. */
static void enforce_applies_recommendations(bool real_module)
{
	const char *what = "enforce, CQP";
	obs_data_t *settings = cqp_settings();

	apply(real_module, settings, false);

	expect_str(what, settings, "rate_control", "CBR");
	expect_int(what, settings, "bitrate", REC_MAX_BITRATE);
	expect_int(what, settings, "buffer_size", REC_MAX_BITRATE);
	expect_int(what, settings, "bf", REC_BFRAMES);
	expect_str(what, settings, "profile", REC_PROFILE);
	expect_str(what, settings, "x264opts", REC_X264OPTS);
	expect_int(what, settings, "keyint_sec", REC_KEYINT);

	obs_data_release(settings);
	check();
}

/* A keyframe interval shorter than the service's is kept either way. */
static void shorter_keyint_is_kept(bool real_module)
{
	for (int ignore = 0; ignore < 2; ignore++) {
		obs_data_t *settings = crf_settings();
		obs_data_set_int(settings, "keyint_sec", 1);
		apply(real_module, settings, ignore);
		expect_int(ignore ? "ignore, keyint 1" : "enforce, keyint 1", settings, "keyint_sec", 1);
		obs_data_release(settings);

		/* A longer one is capped either way. */
		settings = crf_settings();
		obs_data_set_int(settings, "keyint_sec", 10);
		apply(real_module, settings, ignore);
		expect_int(ignore ? "ignore, keyint 10" : "enforce, keyint 10", settings, "keyint_sec", REC_KEYINT);
		obs_data_release(settings);
	}
	check();
}

/* "Ignore" is about the recommendations of the streaming services list.
 * The settings a custom server or WHIP need for their protocol stay. */
static void stand_in_ignore_keeps_protocol_settings(void **state)
{
	(void)state;
	const char *what = "ignore, WHIP";
	obs_data_t *settings = cqp_settings();

	service_encoder_settings_apply_video_with(settings, "whip_custom", true, stand_in_whip_apply, NULL);

	expect_int(what, settings, "bf", 0);
	if (!obs_data_get_bool(settings, "repeat_headers")) {
		fprintf(stderr, "%s: repeat_headers is not set\n", what);
		failures++;
	}
	expect_str(what, settings, "rate_control", "CQP");

	obs_data_release(settings);
	check();
}

static void module_ignore_keeps_protocol_settings(void **state)
{
	(void)state;
	if (!module_up)
		skip();

	const char *what = "ignore, custom SRT server";
	obs_data_t *settings = cqp_settings();

	service_encoder_settings_apply_video(custom_service, settings, true);

	if (!obs_data_get_bool(settings, "repeat_headers")) {
		fprintf(stderr, "%s: repeat_headers is not set\n", what);
		failures++;
	}
	expect_str(what, settings, "rate_control", "CQP");

	obs_data_release(settings);
	check();
}

static void stand_in_ignore_keeps_cqp(void **state)
{
	(void)state;
	ignore_keeps_cqp(false);
}

static void stand_in_ignore_keeps_crf(void **state)
{
	(void)state;
	ignore_keeps_crf(false);
}

static void stand_in_enforce_applies_recommendations(void **state)
{
	(void)state;
	enforce_applies_recommendations(false);
}

static void stand_in_shorter_keyint_is_kept(void **state)
{
	(void)state;
	shorter_keyint_is_kept(false);
}

static void module_ignore_keeps_cqp(void **state)
{
	(void)state;
	if (!module_up)
		skip();
	ignore_keeps_cqp(true);
}

static void module_ignore_keeps_crf(void **state)
{
	(void)state;
	if (!module_up)
		skip();
	ignore_keeps_crf(true);
}

static void module_enforce_applies_recommendations(void **state)
{
	(void)state;
	if (!module_up)
		skip();
	enforce_applies_recommendations(true);
}

static void module_shorter_keyint_is_kept(void **state)
{
	(void)state;
	if (!module_up)
		skip();
	shorter_keyint_is_kept(true);
}

static void quiet_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	if (level <= LOG_WARNING) {
		vfprintf(stderr, format, args);
		fputc('\n', stderr);
	}
}

static bool obs_up;

#ifdef RTMP_SERVICES_MODULE_PATH
/* rtmp-services only lists services whose protocol has an output; this one
 * stands in for obs-outputs' RTMP output and is never started. */
static const char *output_name(void *type_data)
{
	(void)type_data;
	return "RTMP stand-in";
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void output_destroy(void *data)
{
	(void)data;
}

static bool output_start(void *data)
{
	(void)data;
	return false;
}

static void output_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void output_encoded_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}
#endif

static int setup(void **state)
{
	(void)state;

#ifdef RTMP_SERVICES_MODULE_PATH
	base_set_log_handler(quiet_log, NULL);

	/* A services.json with one service that recommends everything
	 * rtmp-services can recommend for video. */
	char json[1024];
	snprintf(json, sizeof(json),
		 "{\"format_version\": %d, \"services\": [{"
		 "\"name\": \"Test Service\", \"common\": true,"
		 "\"servers\": [{\"name\": \"Default\", \"url\": \"rtmp://127.0.0.1/live\"}],"
		 "\"recommended\": {\"keyint\": %d, \"profile\": \"%s\", \"max video bitrate\": %d,"
		 " \"max audio bitrate\": 160, \"bframes\": %d, \"x264opts\": \"%s\"}}]}",
		 RTMP_SERVICES_FORMAT_VERSION, REC_KEYINT, REC_PROFILE, REC_MAX_BITRATE, REC_BFRAMES, REC_X264OPTS);
	if (os_mkdirs(SERVICES_DATA_DIR) == MKDIR_ERROR ||
	    !os_quick_write_utf8_file(SERVICES_DATA_DIR "/services.json", json, strlen(json), false)) {
		fprintf(stderr, "could not write " SERVICES_DATA_DIR "/services.json\n");
		return -1;
	}

	/* The module config path is under a regular file, so rtmp-services
	 * has no config directory: it neither reads a cached services.json
	 * nor starts its service update download. */
	obs_up = obs_startup("en-US", SERVICES_DATA_DIR "/services.json", NULL);
	if (!obs_up) {
		fprintf(stderr, "module_ cases skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_output_info info = {
		.id = "test_rtmp_output",
		.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_SERVICE,
		.get_name = output_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_encoded_packet,
		.encoded_video_codecs = "h264",
		.encoded_audio_codecs = "aac",
		.protocols = "RTMP",
	};
	obs_register_output(&info);

	obs_module_t *module = NULL;
	if (obs_open_module(&module, RTMP_SERVICES_MODULE_PATH, SERVICES_DATA_DIR) != MODULE_SUCCESS ||
	    !obs_init_module(module)) {
		fprintf(stderr, "could not load rtmp-services\n");
		return -1;
	}

	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "service", "Test Service");
	obs_data_set_string(settings, "server", "rtmp://127.0.0.1/live");
	obs_data_set_string(settings, "key", "test");
	service = obs_service_create("rtmp_common", "test service", settings, NULL);
	obs_data_release(settings);
	if (!service) {
		fprintf(stderr, "could not create the rtmp_common service\n");
		return -1;
	}

	settings = obs_data_create();
	obs_data_set_string(settings, "server", "srt://127.0.0.1:9000");
	custom_service = obs_service_create("rtmp_custom", "test custom service", settings, NULL);
	obs_data_release(settings);
	if (!custom_service) {
		fprintf(stderr, "could not create the rtmp_custom service\n");
		return -1;
	}
	module_up = true;
#else
	(void)quiet_log;
	fprintf(stderr, "module_ cases skipped: rtmp-services is not a loadable module here\n");
#endif
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_service_release(custom_service);
	obs_service_release(service);
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(stand_in_ignore_keeps_cqp),
		cmocka_unit_test(stand_in_ignore_keeps_crf),
		cmocka_unit_test(stand_in_enforce_applies_recommendations),
		cmocka_unit_test(stand_in_shorter_keyint_is_kept),
		cmocka_unit_test(stand_in_ignore_keeps_protocol_settings),
		cmocka_unit_test(module_ignore_keeps_cqp),
		cmocka_unit_test(module_ignore_keeps_crf),
		cmocka_unit_test(module_enforce_applies_recommendations),
		cmocka_unit_test(module_shorter_keyint_is_kept),
		cmocka_unit_test(module_ignore_keeps_protocol_settings),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}

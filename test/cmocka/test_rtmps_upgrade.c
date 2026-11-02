/* rtmp-services connects to built-in services over RTMPS when their ingest
 * server is known to accept it (obs-rust/obs-studio#2205). The service still
 * stores the plain rtmp:// server the user picked; the upgrade happens when the
 * output asks for the URL, and the service keeps the plain URL so the frontend
 * can offer it if the encrypted connection fails.
 *
 * The cases load the real rtmp-services module with a services.json written
 * by the test. They need obs_startup(), which needs a display on Linux, so
 * they skip without one (Tier 2 runs ctest under xvfb-run). */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <obs-module.h>
#include <callback/proc.h>
#include <util/platform.h>

#ifdef RTMP_SERVICES_MODULE_PATH
#include "rtmp-format-ver.h"
#endif

/* An ingest server that accepts RTMPS on port 443 and is on the list. */
#define LISTED_PLAIN "rtmp://live.twitch.tv/app"
#define LISTED_RTMPS "rtmps://live.twitch.tv:443/app"

static bool module_up;

static void quiet_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
}

static obs_service_t *make_service(const char *id, const char *service, const char *server)
{
	obs_data_t *settings = obs_data_create();
	if (service)
		obs_data_set_string(settings, "service", service);
	obs_data_set_string(settings, "server", server);
	obs_data_set_string(settings, "key", "test");
	obs_service_t *s = obs_service_create(id, "test service", settings, NULL);
	obs_data_release(settings);
	return s;
}

static const char *url_of(obs_service_t *s)
{
	return obs_service_get_connect_info(s, OBS_SERVICE_CONNECT_INFO_SERVER_URL);
}

/* The plain URL the service would fall back to, or "" when it did not
 * upgrade the URL (or the module has no fallback command). */
static void fallback_of(obs_service_t *s, char *buf, size_t size)
{
	calldata_t cd = {0};
	const char *plain = NULL;

	buf[0] = '\0';
	calldata_set_ptr(&cd, "service", s);
	if (proc_handler_call(obs_get_proc_handler(), "rtmp_services_rtmps_fallback", &cd) &&
	    calldata_get_string(&cd, "plain_url", &plain) && plain)
		snprintf(buf, size, "%s", plain);
	calldata_free(&cd);
}

static bool set_plain_rtmp(obs_service_t *s, bool enabled)
{
	calldata_t cd = {0};
	calldata_set_ptr(&cd, "service", s);
	calldata_set_bool(&cd, "enabled", enabled);
	bool ok = proc_handler_call(obs_get_proc_handler(), "rtmp_services_set_plain_rtmp", &cd);
	calldata_free(&cd);
	return ok;
}

#define SKIP_WITHOUT_MODULE()                                                        \
	do {                                                                         \
		if (!module_up) {                                                    \
			print_message("rtmp-services module not loaded, skipping\n"); \
			skip();                                                      \
		}                                                                    \
	} while (0)

/* A built-in service on a listed host connects over RTMPS on port 443, with
 * the same host and app path. */
static void module_listed_server_connects_over_rtmps(void **state)
{
	(void)state;
	SKIP_WITHOUT_MODULE();

	obs_service_t *s = make_service("rtmp_common", "Twitch", LISTED_PLAIN);
	assert_non_null(s);
	assert_string_equal(url_of(s), LISTED_RTMPS);
	obs_service_release(s);
}

/* The service keeps the plain URL so the frontend can offer it. */
static void module_listed_server_reports_plain_fallback(void **state)
{
	(void)state;
	SKIP_WITHOUT_MODULE();

	char plain[256];
	obs_service_t *s = make_service("rtmp_common", "Twitch", LISTED_PLAIN);
	assert_non_null(s);
	fallback_of(s, plain, sizeof(plain));
	assert_string_equal(plain, LISTED_PLAIN);
	obs_service_release(s);
}

/* After the user accepts the fallback, the service hands out the plain URL
 * until the frontend clears it for the next start. */
static void module_plain_rtmp_after_fallback(void **state)
{
	(void)state;
	SKIP_WITHOUT_MODULE();

	obs_service_t *s = make_service("rtmp_common", "Twitch", LISTED_PLAIN);
	assert_non_null(s);
	assert_true(set_plain_rtmp(s, true));
	assert_string_equal(url_of(s), LISTED_PLAIN);
	assert_true(set_plain_rtmp(s, false));
	assert_string_equal(url_of(s), LISTED_RTMPS);
	obs_service_release(s);
}

/* A host that is not on the list stays on plain RTMP and has no fallback. */
static void module_unlisted_host_stays_plain(void **state)
{
	(void)state;
	SKIP_WITHOUT_MODULE();

	char plain[256];
	obs_service_t *s = make_service("rtmp_common", "Twitch", "rtmp://127.0.0.1/app");
	assert_non_null(s);
	assert_string_equal(url_of(s), "rtmp://127.0.0.1/app");
	fallback_of(s, plain, sizeof(plain));
	assert_string_equal(plain, "");
	obs_service_release(s);
}

/* The list is per service: a listed host under another service name stays
 * plain. */
static void module_unlisted_service_stays_plain(void **state)
{
	(void)state;
	SKIP_WITHOUT_MODULE();

	obs_service_t *s = make_service("rtmp_common", "Test Service", LISTED_PLAIN);
	assert_non_null(s);
	assert_string_equal(url_of(s), LISTED_PLAIN);
	obs_service_release(s);
}

/* Only the default RTMP port is mapped to 443. */
static void module_other_port_stays_plain(void **state)
{
	(void)state;
	SKIP_WITHOUT_MODULE();

	obs_service_t *s = make_service("rtmp_common", "Twitch", "rtmp://live.twitch.tv:1936/app");
	assert_non_null(s);
	assert_string_equal(url_of(s), "rtmp://live.twitch.tv:1936/app");
	obs_service_release(s);

	s = make_service("rtmp_common", "Twitch", "rtmp://live.twitch.tv:1935/app");
	assert_non_null(s);
	assert_string_equal(url_of(s), LISTED_RTMPS);
	obs_service_release(s);
}

/* The host match ignores case, and the query string is kept. */
static void module_host_case_and_query(void **state)
{
	(void)state;
	SKIP_WITHOUT_MODULE();

	obs_service_t *s = make_service("rtmp_common", "Twitch", "rtmp://LIVE.Twitch.TV/app?x=1");
	assert_non_null(s);
	assert_string_equal(url_of(s), "rtmps://LIVE.Twitch.TV:443/app?x=1");
	obs_service_release(s);
}

/* A server the user typed into a custom service is never changed. */
static void module_custom_server_unchanged(void **state)
{
	(void)state;
	SKIP_WITHOUT_MODULE();

	char plain[256];
	obs_service_t *s = make_service("rtmp_custom", NULL, LISTED_PLAIN);
	assert_non_null(s);
	assert_string_equal(url_of(s), LISTED_PLAIN);
	fallback_of(s, plain, sizeof(plain));
	assert_string_equal(plain, "");
	obs_service_release(s);
}

/* A server that is already rtmps:// is used as is, with no fallback. */
static void module_rtmps_server_has_no_fallback(void **state)
{
	(void)state;
	SKIP_WITHOUT_MODULE();

	char plain[256];
	obs_service_t *s = make_service("rtmp_common", "Twitch", LISTED_RTMPS);
	assert_non_null(s);
	assert_string_equal(url_of(s), LISTED_RTMPS);
	fallback_of(s, plain, sizeof(plain));
	assert_string_equal(plain, "");
	obs_service_release(s);
}

#ifdef RTMP_SERVICES_MODULE_PATH
/* rtmp-services only upgrades when an output handles RTMPS; this stands in
 * for obs-outputs' RTMP output and is never started. */
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

static int setup(void)
{
#ifdef RTMP_SERVICES_MODULE_PATH
	base_set_log_handler(quiet_log, NULL);

	char json[1024];
	snprintf(json, sizeof(json),
		 "{\"format_version\": %d, \"services\": ["
		 "{\"name\": \"Twitch\", \"common\": true, \"servers\": ["
		 "{\"name\": \"Default\", \"url\": \"" LISTED_PLAIN "\"},"
		 "{\"name\": \"Local\", \"url\": \"rtmp://127.0.0.1/app\"}]},"
		 "{\"name\": \"Test Service\", \"common\": true, \"servers\": ["
		 "{\"name\": \"Default\", \"url\": \"" LISTED_PLAIN "\"}]}]}",
		 RTMP_SERVICES_FORMAT_VERSION);

	if (os_mkdirs(SERVICES_DATA_DIR) == MKDIR_ERROR ||
	    !os_quick_write_utf8_file(SERVICES_DATA_DIR "/services.json", json, strlen(json), false)) {
		fprintf(stderr, "could not write " SERVICES_DATA_DIR "/services.json\n");
		return -1;
	}

	/* The module config path is under a regular file, so rtmp-services
	 * has no config directory: it neither reads a cached services.json
	 * nor starts its service update download. */
	if (!obs_startup("en-US", SERVICES_DATA_DIR "/services.json", NULL)) {
		fprintf(stderr, "cases skipped: obs_startup failed (no display?)\n");
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
		.protocols = "RTMP;RTMPS",
	};
	obs_register_output(&info);

	obs_module_t *module = NULL;
	if (obs_open_module(&module, RTMP_SERVICES_MODULE_PATH, SERVICES_DATA_DIR) != MODULE_SUCCESS ||
	    !obs_init_module(module)) {
		fprintf(stderr, "could not load rtmp-services\n");
		return -1;
	}

	module_up = true;
#else
	(void)quiet_log;
	fprintf(stderr, "cases skipped: rtmp-services is not a loadable module here\n");
#endif
	return 0;
}

int main(void)
{
	if (setup() != 0)
		return 1;

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(module_listed_server_connects_over_rtmps),
		cmocka_unit_test(module_listed_server_reports_plain_fallback),
		cmocka_unit_test(module_plain_rtmp_after_fallback),
		cmocka_unit_test(module_unlisted_host_stays_plain),
		cmocka_unit_test(module_unlisted_service_stays_plain),
		cmocka_unit_test(module_other_port_stays_plain),
		cmocka_unit_test(module_host_case_and_query),
		cmocka_unit_test(module_custom_server_unchanged),
		cmocka_unit_test(module_rtmps_server_has_no_fallback),
	};
	int failed = cmocka_run_group_tests(tests, NULL, NULL);

	if (module_up)
		obs_shutdown();
	return failed;
}

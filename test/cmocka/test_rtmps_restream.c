/* Restream documents live.restream.io:1937 in its RTMPS encoder guide.
 * Exercise the production mapper without an account or network connection. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include "plugins/rtmp-services/service-specific/rtmps-upgrade.c.h"

static void autodetect_uses_documented_rtmps_port(void **state)
{
	(void)state;
	struct dstr result = {0};
	assert_true(rtmps_upgrade_url("Restream.io", "rtmp://live.restream.io/live", &result));
	assert_string_equal(result.array, "rtmps://live.restream.io:1937/live");
	dstr_free(&result);
}

static void default_port_case_and_query_are_preserved(void **state)
{
	(void)state;
	struct dstr result = {0};
	assert_true(rtmps_upgrade_url("Restream.io", "RTMP://LIVE.restream.io:1935/live?x=1", &result));
	assert_string_equal(result.array, "rtmps://LIVE.restream.io:1937/live?x=1");
	dstr_free(&result);
}

static void unverified_urls_are_unchanged(void **state)
{
	(void)state;
	const char *urls[] = {
		"rtmp://london.restream.io/live",           "rtmp://sanjose.restream.io/live",
		"rtmp://live.restream.io.example.com/live", "rtmp://live.restream.io:80/live",
		"rtmp://live.restream.io:443/live",         "rtmp://live.restream.io:1937/live",
		"rtmps://live.restream.io:1937/live",
	};
	struct dstr result = {0};
	for (size_t i = 0; i < sizeof(urls) / sizeof(urls[0]); i++)
		assert_false(rtmps_upgrade_url("Restream.io", urls[i], &result));
	assert_false(rtmps_upgrade_url("Other service", "rtmp://live.restream.io/live", &result));
	assert_false(rtmps_upgrade_url(NULL, "rtmp://live.restream.io/live", &result));
	assert_false(rtmps_upgrade_url("Restream.io", NULL, &result));
	assert_null(result.array);
}

static void existing_services_keep_their_ports(void **state)
{
	(void)state;
	struct dstr result = {0};
	assert_true(rtmps_upgrade_url("Twitch", "rtmp://live.twitch.tv/app", &result));
	assert_string_equal(result.array, "rtmps://live.twitch.tv:443/app");
	assert_true(rtmps_upgrade_url("nanoStream Cloud / bintu", "rtmp://bintu-stream.nanocosmos.de/live", &result));
	assert_string_equal(result.array, "rtmps://bintu-stream.nanocosmos.de:1937/live");
	dstr_free(&result);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(autodetect_uses_documented_rtmps_port),
		cmocka_unit_test(default_port_case_and_query_are_preserved),
		cmocka_unit_test(unverified_urls_are_unchanged),
		cmocka_unit_test(existing_services_keep_their_ports),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

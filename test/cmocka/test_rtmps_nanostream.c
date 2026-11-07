/* Pin the verified nanoStream global ingest mapping (#2216). No network or
 * account is needed: this exercises the production URL decision directly. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <cmocka.h>

#include "plugins/rtmp-services/service-specific/rtmps-upgrade.c.h"

static void global_ingests_use_port_1937(void **state)
{
	(void)state;
	const char *hosts[] = {"bintu-stream.nanocosmos.de", "bintu-vtrans.nanocosmos.de"};
	for (size_t i = 0; i < sizeof(hosts) / sizeof(hosts[0]); i++) {
		char plain[256], secure[256];
		struct dstr result = {0};
		snprintf(plain, sizeof(plain), "rtmp://%s/live", hosts[i]);
		snprintf(secure, sizeof(secure), "rtmps://%s:1937/live", hosts[i]);
		bool upgraded = rtmps_upgrade_url("nanoStream Cloud / bintu", plain, &result);
		assert_true(upgraded);
		assert_string_equal(result.array, secure);
		dstr_free(&result);
	}
}

static void default_port_case_and_query_are_preserved(void **state)
{
	(void)state;
	struct dstr result = {0};
	assert_true(rtmps_upgrade_url("nanoStream Cloud / bintu", "RTMP://BINTU-stream.nanocosmos.de:1935/live?x=1",
				      &result));
	assert_string_equal(result.array, "rtmps://BINTU-stream.nanocosmos.de:1937/live?x=1");
	dstr_free(&result);
}

static void unverified_urls_are_unchanged(void **state)
{
	(void)state;
	const char *urls[] = {
		"rtmp://bintu-stream.nanocosmos.de:80/live",          "rtmp://bintu-stream.nanocosmos.de:1937/live",
		"rtmps://bintu-stream.nanocosmos.de:1937/live",       "rtmp://bintu-stream-eu.nanocosmos.de/live",
		"rtmp://bintu-stream.nanocosmos.de.example.com/live",
	};
	struct dstr result = {0};
	for (size_t i = 0; i < sizeof(urls) / sizeof(urls[0]); i++)
		assert_false(rtmps_upgrade_url("nanoStream Cloud / bintu", urls[i], &result));
	assert_false(rtmps_upgrade_url("Other service", "rtmp://bintu-stream.nanocosmos.de/live", &result));
	assert_null(result.array);
}

static void existing_services_keep_port_443(void **state)
{
	(void)state;
	struct dstr result = {0};
	assert_true(rtmps_upgrade_url("Twitch", "rtmp://live.twitch.tv/app", &result));
	assert_string_equal(result.array, "rtmps://live.twitch.tv:443/app");
	dstr_free(&result);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(global_ingests_use_port_1937),
		cmocka_unit_test(default_port_case_and_query_are_preserved),
		cmocka_unit_test(unverified_urls_are_unchanged),
		cmocka_unit_test(existing_services_keep_port_443),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

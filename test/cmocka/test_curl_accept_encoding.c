/*
 * The Accept-Encoding our outgoing HTTP calls send (#2211, obs-rust/obs-studio#2453).
 *
 * Where the linked Brotli is one this repo pins, or is not linked at all, the
 * calls ask for everything the libcurl decodes ("") so servers may answer with
 * br again. Where the frontend uses the OS libcurl (Apple), the stop-gap
 * stays: gzip and deflate only, never br. A libcurl without zlib must still
 * never be asked for gzip: asking for it made Windows' service list, capture
 * list and YouTube calls fail with CURLE_BAD_CONTENT_ENCODING (#2453).
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <string.h>

#include "shared/file-updater/file-updater/curl-accept-encoding.h"

static bool lists_coding(const char *value, const char *token)
{
	return value && strstr(value, token) != NULL;
}

#if defined(__APPLE__)
/* Apple: the stop-gap, never br. */
static void apple_libcurl_keeps_the_stop_gap(void **state)
{
	(void)state;
	curl_version_info_data info = {0};
	info.features = CURL_VERSION_LIBZ | CURL_VERSION_BROTLI;
	const char *enc = obs_curl_accept_encoding(&info);
	assert_string_equal(enc, "gzip, deflate");
	assert_false(lists_coding(enc, "br"));
}

static void no_zlib_without_a_pinned_brotli_asks_for_identity(void **state)
{
	(void)state;
	curl_version_info_data info = {0};
	info.features = CURL_VERSION_SSL; /* Windows: Schannel, no zlib */
	assert_string_equal(obs_curl_accept_encoding(&info), "identity");
	info.features |= CURL_VERSION_BROTLI;
	assert_string_equal(obs_curl_accept_encoding(&info), "identity");
	assert_string_equal(obs_curl_accept_encoding(NULL), "identity");
}
#else
/* Every coding the linked libcurl decodes, br included (#2211). */
static void pinned_brotli_asks_for_every_coding(void **state)
{
	(void)state;
	curl_version_info_data info = {0};
	info.features = CURL_VERSION_LIBZ | CURL_VERSION_BROTLI;
	assert_string_equal(obs_curl_accept_encoding(&info), "");
	info.features = CURL_VERSION_BROTLI;
	assert_string_equal(obs_curl_accept_encoding(&info), "");
	assert_string_equal(obs_curl_accept_encoding(NULL), "");
}

static void this_platform_asks_for_every_coding(void **state)
{
	(void)state;
	assert_string_equal(obs_curl_accept_encoding(curl_version_info(CURLVERSION_NOW)), "");
}
#endif

int main(void)
{
	const struct CMUnitTest tests[] = {
#if defined(__APPLE__)
		cmocka_unit_test(apple_libcurl_keeps_the_stop_gap),
		cmocka_unit_test(no_zlib_without_a_pinned_brotli_asks_for_identity),
#else
		cmocka_unit_test(pinned_brotli_asks_for_every_coding),
		cmocka_unit_test(this_platform_asks_for_every_coding),
#endif
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

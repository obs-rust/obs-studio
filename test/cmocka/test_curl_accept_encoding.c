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

/* Non-Apple: every coding the linked libcurl decodes, br included. */
static void pinned_brotli_asks_for_every_coding(void **state)
{
	(void)state;
	curl_version_info_data info = {0};
	info.features = CURL_VERSION_LIBZ | CURL_VERSION_BROTLI;
	assert_string_equal(obs_curl_accept_encoding_for(&info, 0), "");
	info.features = CURL_VERSION_BROTLI;
	assert_string_equal(obs_curl_accept_encoding_for(&info, 0), "");
	assert_string_equal(obs_curl_accept_encoding_for(NULL, 0), "");
}

/* Apple: the stop-gap. */
static void unpinned_brotli_asks_for_gzip_and_deflate(void **state)
{
	(void)state;
	curl_version_info_data info = {0};
	info.features = CURL_VERSION_LIBZ | CURL_VERSION_BROTLI;
	const char *enc = obs_curl_accept_encoding_for(&info, 1);
	assert_string_equal(enc, "gzip, deflate");
	assert_false(lists_coding(enc, "br"));
}

static void unpinned_brotli_without_zlib_asks_for_identity(void **state)
{
	(void)state;
	curl_version_info_data info = {0};
	info.features = CURL_VERSION_SSL; /* Windows: Schannel, no zlib */
	assert_string_equal(obs_curl_accept_encoding_for(&info, 1), "identity");
	info.features |= CURL_VERSION_BROTLI;
	assert_string_equal(obs_curl_accept_encoding_for(&info, 1), "identity");
	assert_string_equal(obs_curl_accept_encoding_for(NULL, 1), "identity");
}

/* The value the call sites get on the platform this binary was built for. */
static void this_platform_gets_its_coding(void **state)
{
	(void)state;
	const curl_version_info_data *info = curl_version_info(CURLVERSION_NOW);
	const char *enc = obs_curl_accept_encoding(info);

#if OBS_CURL_ACCEPT_ENCODING_UNPINNED_BROTLI
	if (info->features & CURL_VERSION_LIBZ)
		assert_string_equal(enc, "gzip, deflate");
	else
		assert_string_equal(enc, "identity");
	assert_false(lists_coding(enc, "br"));
#else
	assert_string_equal(enc, "");
#endif
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(pinned_brotli_asks_for_every_coding),
		cmocka_unit_test(unpinned_brotli_asks_for_gzip_and_deflate),
		cmocka_unit_test(unpinned_brotli_without_zlib_asks_for_identity),
		cmocka_unit_test(this_platform_gets_its_coding),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

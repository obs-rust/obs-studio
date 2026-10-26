/*
 * The Accept-Encoding our outgoing HTTP calls send (obs-rust/obs-studio#2453).
 * A libcurl without zlib cannot decode gzip: asking for it made Windows'
 * service list, capture list and YouTube calls fail with
 * CURLE_BAD_CONTENT_ENCODING. br is never asked for (#2213).
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

static void with_zlib_asks_for_gzip_and_deflate(void **state)
{
	(void)state;
	curl_version_info_data info = {0};
	info.features = CURL_VERSION_LIBZ | CURL_VERSION_BROTLI;
	const char *enc = obs_curl_accept_encoding(&info);
	assert_string_equal(enc, "gzip, deflate");
	assert_false(lists_coding(enc, "br"));
}

static void without_zlib_asks_for_identity_only(void **state)
{
	(void)state;
	curl_version_info_data info = {0};
	info.features = CURL_VERSION_SSL; /* Windows: Schannel, no zlib */
	assert_string_equal(obs_curl_accept_encoding(&info), "identity");
	info.features |= CURL_VERSION_BROTLI;
	assert_string_equal(obs_curl_accept_encoding(&info), "identity");
}

static void no_version_info_asks_for_identity(void **state)
{
	(void)state;
	assert_string_equal(obs_curl_accept_encoding(NULL), "identity");
}

static void linked_libcurl_gets_a_coding_it_decodes(void **state)
{
	(void)state;
	const curl_version_info_data *info = curl_version_info(CURLVERSION_NOW);
	const char *enc = obs_curl_accept_encoding(info);
	if (info->features & CURL_VERSION_LIBZ)
		assert_string_equal(enc, "gzip, deflate");
	else
		assert_string_equal(enc, "identity");
	assert_false(lists_coding(enc, "br"));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(with_zlib_asks_for_gzip_and_deflate),
		cmocka_unit_test(without_zlib_asks_for_identity_only),
		cmocka_unit_test(no_version_info_asks_for_identity),
		cmocka_unit_test(linked_libcurl_gets_a_coding_it_decodes),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

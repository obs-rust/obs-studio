#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include "obs-ffmpeg-rist.h"

/* Exercise the plugin's static functions directly, with external librist
 * calls interposed here. No sender thread or network connection is created.
 */
static int sender_token;
static unsigned destroy_calls;

int rist_sender_create(struct rist_ctx **ctx, enum rist_profile profile, uint32_t flow_id,
		       struct rist_logging_settings *logging_settings)
{
	UNUSED_PARAMETER(profile);
	UNUSED_PARAMETER(flow_id);
	UNUSED_PARAMETER(logging_settings);
	*ctx = (struct rist_ctx *)&sender_token;
	return 0;
}

/* Plain statics rather than cmocka's will_return/expect queues: those crashed
 * inside cmocka's _mock on the MSVC build. */
static const char *parse_expected_url;
static int parse_ret_value;
static unsigned parse_calls;

static int parse_address(const char *url, struct rist_peer_config *config)
{
	parse_calls++;
	assert_string_equal(url, parse_expected_url);
	if (parse_ret_value == 0)
		config->key_size = 128;
	return parse_ret_value;
}

#if FF_LIBRIST_VERSION < FF_LIBRIST_VERSION_41
int rist_parse_address(const char *url, const struct rist_peer_config **config)
{
	return parse_address(url, (struct rist_peer_config *)*config);
}
#else
int rist_parse_address2(const char *url, struct rist_peer_config **config)
{
	return parse_address(url, *config);
}
#endif

int rist_destroy(struct rist_ctx *ctx)
{
	assert_ptr_equal(ctx, &sender_token);
	destroy_calls++;
	return 0;
}

static void open_failure_cleanup(const char *url, int parse_ret, int expected_ret)
{
	long allocations = bnum_allocs();
	RISTContext context = {
		.secret = bstrdup("service-passphrase"),
		.username = bstrdup("service-user"),
		.password = bstrdup("service-password"),
	};
	URLContext h = {.priv_data = &context, .url = (char *)url};
	destroy_calls = 0;
	assert_int_equal(bnum_allocs(), allocations + 3);

	parse_expected_url = url;
	parse_ret_value = parse_ret;
	parse_calls = 0;
	assert_int_equal(librist_open(&h, url), expected_ret);
	assert_int_equal(parse_calls, 1);
	assert_int_equal(destroy_calls, 1);
	assert_int_equal(bnum_allocs(), allocations);

	/* Check consumed credentials before the caller's cleanup, so RED fails
	 * deterministically rather than depending on an allocator's double-free
	 * diagnostics. connect_mpegts_url's fail path then closes the same context.
	 */
	assert_null(context.secret);
	assert_null(context.username);
	assert_null(context.password);
	assert_null(context.peer);
	assert_null(context.ctx);
	assert_int_equal(librist_close(&h), 0);
	assert_int_equal(destroy_calls, 1);
	assert_int_equal(bnum_allocs(), allocations);
}

static void parse_failure_cleanup_test(void **state)
{
	UNUSED_PARAMETER(state);
	open_failure_cleanup("rist://invalid", -1, OBS_OUTPUT_BAD_PATH);
}

static void encrypted_url_cleanup_test(void **state)
{
	UNUSED_PARAMETER(state);
	/* A service passphrase is present, but the URL configuration has no secret.
	 * This reaches librist_open's mandatory-secret failure after URL parsing.
	 */
	open_failure_cleanup("rist://127.0.0.1:9000?aes-type=128", 0, OBS_OUTPUT_INVALID_STREAM);
}

static void repeated_close_test(void **state)
{
	UNUSED_PARAMETER(state);
	long allocations = bnum_allocs();
	RISTContext context = {
		.secret = bstrdup("secret"),
		.username = bstrdup("user"),
		.password = bstrdup("password"),
		.ctx = (struct rist_ctx *)&sender_token,
	};
	URLContext h = {.priv_data = &context, .url = "rist://127.0.0.1:9000"};
	destroy_calls = 0;
	assert_int_equal(librist_close(&h), 0);
	assert_int_equal(bnum_allocs(), allocations);
	assert_null(context.secret);
	assert_null(context.username);
	assert_null(context.password);
	assert_null(context.ctx);
	assert_int_equal(librist_close(&h), 0);
	assert_int_equal(destroy_calls, 1);
	assert_int_equal(bnum_allocs(), allocations);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(parse_failure_cleanup_test),
		cmocka_unit_test(encrypted_url_cleanup_test),
		cmocka_unit_test(repeated_close_test),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

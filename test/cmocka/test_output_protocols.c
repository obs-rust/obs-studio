/*
 * Characterization of the output protocol lookups through the public API,
 * before the protocol-list match of libobs/obs-output.c
 * (obs_output_protocol_list_count() in obs-output-protocols.c) is ported to
 * rust/obs-output:
 * - obs_register_output() adds each non-empty ';'-separated segment of a
 *   service output's protocols to the registered protocols, once, in
 *   registration order (obs-module.c);
 * - obs_is_output_protocol_registered() and obs_enum_output_protocols()
 *   read that list (obs.c);
 * - obs_enum_output_types_with_protocol() calls enum_cb with the id of each
 *   service output whose list holds the protocol, once per matching segment,
 *   in registration order, and stops when enum_cb returns false
 *   (obs-output.c);
 * - obs_output_get_protocols() and obs_get_output_flags() return what the
 *   output registered.
 * They pin the current C behavior; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * Public API only, so it also runs the hybrid libobs. obs_startup() loads no
 * modules here, so the outputs below are the only ones registered. It needs
 * a display on Linux (xvfb-run), and the cases skip without one.
 *
 * A NULL protocol is not driven: obs_is_output_protocol_registered(NULL) and
 * obs_enum_output_types_with_protocol(NULL, ...) crash in strcmp() on this C,
 * a separate fix.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <obs.h>
#include <obs-module.h>

static bool obs_up;

static const char *dummy_get_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "Protocol Test Output";
}

static void *dummy_create(obs_data_t *settings, obs_output_t *output)
{
	UNUSED_PARAMETER(settings);
	UNUSED_PARAMETER(output);
	return NULL;
}

static void dummy_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool dummy_start(void *data)
{
	UNUSED_PARAMETER(data);
	return false;
}

static void dummy_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(ts);
}

static void dummy_encoded_packet(void *data, struct encoder_packet *packet)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(packet);
}

#define SERVICE_FLAGS (OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_SERVICE)
#define PLAIN_FLAGS (OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED)

#define OUTPUT(ident, list, output_flags)                \
	{                                                \
		.id = ident,                             \
		.flags = output_flags,                   \
		.protocols = list,                       \
		.encoded_video_codecs = "h264",          \
		.encoded_audio_codecs = "aac",           \
		.get_name = dummy_get_name,              \
		.create = dummy_create,                  \
		.destroy = dummy_destroy,                \
		.start = dummy_start,                    \
		.stop = dummy_stop,                      \
		.encoded_packet = dummy_encoded_packet,  \
	}

/* Registered in this order. */
static const struct obs_output_info outputs[] = {
	OUTPUT("proto_dup", "RTMP;RTMP", SERVICE_FLAGS),
	OUTPUT("proto_rtmp_rtmps", "RTMP;RTMPS", SERVICE_FLAGS),
	OUTPUT("proto_srt", "SRT", SERVICE_FLAGS),
	OUTPUT("proto_empty_segments", ";RTMP;;", SERVICE_FLAGS),
	OUTPUT("proto_prefix", "RTMPX", SERVICE_FLAGS),
	OUTPUT("proto_lower", "rtmp", SERVICE_FLAGS),
	OUTPUT("proto_trailing", "HLS;", SERVICE_FLAGS),
	OUTPUT("proto_space", "WHIP; RIST", SERVICE_FLAGS),
	OUTPUT("proto_empty", "", SERVICE_FLAGS),
	/* not a service output: its protocols are neither registered nor
	 * enumerated */
	OUTPUT("proto_plain", "RTMP;PLAINONLY", PLAIN_FLAGS),
};

#define NUM_OUTPUTS (sizeof(outputs) / sizeof(outputs[0]))

/* ------------------------------------------------------------------------ */
/* enum_cb: records the ids it is called with, and stops after stop_after   */

#define MAX_CALLS 16

struct calls {
	const char *ids[MAX_CALLS];
	size_t num;
	size_t stop_after; /* 0: never stop */
};

static bool record_cb(void *data, const char *id)
{
	struct calls *calls = data;
	assert_true(calls->num < MAX_CALLS);
	calls->ids[calls->num++] = id;
	return calls->stop_after == 0 || calls->num < calls->stop_after;
}

static void expect_enum(const char *protocol, size_t stop_after, const char *const *expected, size_t num_expected)
{
	struct calls calls = {.stop_after = stop_after};
	obs_enum_output_types_with_protocol(protocol, &calls, record_cb);
	if (calls.num != num_expected) {
		fprintf(stderr, "protocol \"%s\" (stop after %zu): %zu calls, expected %zu\n", protocol, stop_after,
			calls.num, num_expected);
		for (size_t i = 0; i < calls.num; i++)
			fprintf(stderr, "  call %zu: %s\n", i, calls.ids[i]);
	}
	assert_int_equal(calls.num, num_expected);
	for (size_t i = 0; i < num_expected; i++)
		assert_string_equal(calls.ids[i], expected[i]);
}

#define EXPECT_ENUM(protocol, stop_after, ...)                                                \
	do {                                                                                  \
		static const char *const expected_[] = {__VA_ARGS__};                         \
		expect_enum(protocol, stop_after, expected_, sizeof(expected_) / sizeof(expected_[0])); \
	} while (0)

#define EXPECT_NO_ENUM(protocol) expect_enum(protocol, 0, NULL, 0)

/* ------------------------------------------------------------------------ */

/* The registered protocols: each non-empty segment once, in registration
 * order, case and spaces kept. */
static void registered_protocol_list(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	static const char *const expected[] = {"RTMP", "RTMPS", "SRT", "RTMPX", "rtmp", "HLS", "WHIP", " RIST"};
	const size_t num_expected = sizeof(expected) / sizeof(expected[0]);

	char *protocol = NULL;
	size_t idx = 0;
	while (obs_enum_output_protocols(idx, &protocol)) {
		assert_true(idx < num_expected);
		assert_non_null(protocol);
		assert_string_equal(protocol, expected[idx]);
		idx++;
	}
	assert_int_equal(idx, num_expected);

	/* past the end: false, and the out pointer is left alone */
	protocol = (char *)"untouched";
	assert_false(obs_enum_output_protocols(num_expected, &protocol));
	assert_string_equal(protocol, "untouched");
	assert_false(obs_enum_output_protocols((size_t)-1, &protocol));
	assert_string_equal(protocol, "untouched");
}

static void is_registered(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	static const char *const yes[] = {"RTMP", "RTMPS", "SRT", "RTMPX", "rtmp", "HLS", "WHIP", " RIST"};
	for (size_t i = 0; i < sizeof(yes) / sizeof(yes[0]); i++) {
		if (!obs_is_output_protocol_registered(yes[i]))
			fprintf(stderr, "\"%s\" not registered\n", yes[i]);
		assert_true(obs_is_output_protocol_registered(yes[i]));
	}

	/* whole, case-sensitive names; the separator and empty segments are not
	 * protocols, and a plain output's list is not registered */
	static const char *const no[] = {"",     "Rtmp", "RTM",       "RTMP;RTMPS", "RTMP;", ";",     "HLS;",
					 "RIST", " ",    "PLAINONLY", "FOO",        "srt",   "RTMP ", "rtmps"};
	for (size_t i = 0; i < sizeof(no) / sizeof(no[0]); i++) {
		if (obs_is_output_protocol_registered(no[i]))
			fprintf(stderr, "\"%s\" registered\n", no[i]);
		assert_false(obs_is_output_protocol_registered(no[i]));
	}
}

/* Each matching segment calls enum_cb once, in registration order. */
static void enum_order_and_duplicates(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	/* Characterized, not endorsed: "RTMP;RTMP" lists one output twice. */
	EXPECT_ENUM("RTMP", 0, "proto_dup", "proto_dup", "proto_rtmp_rtmps", "proto_empty_segments");
	EXPECT_ENUM("RTMPS", 0, "proto_rtmp_rtmps");
	EXPECT_ENUM("SRT", 0, "proto_srt");
	EXPECT_ENUM("HLS", 0, "proto_trailing");
	EXPECT_ENUM("WHIP", 0, "proto_space");
	EXPECT_ENUM(" RIST", 0, "proto_space");
}

/* "RTMP" does not match "RTMPX" or "rtmp", and "RTMPX" does not match
 * "RTMP". */
static void enum_whole_names(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	EXPECT_ENUM("RTMPX", 0, "proto_prefix");
	EXPECT_ENUM("rtmp", 0, "proto_lower");
}

/* An unregistered protocol enumerates nothing, even one a plain output
 * lists; so does the empty protocol. */
static void enum_unregistered(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	EXPECT_NO_ENUM("FOO");
	EXPECT_NO_ENUM("");
	EXPECT_NO_ENUM("Rtmp");
	EXPECT_NO_ENUM("RIST");
	EXPECT_NO_ENUM("PLAINONLY");
	EXPECT_NO_ENUM("RTMP;RTMPS");
	EXPECT_NO_ENUM(";");
}

/* enum_cb returning false stops the enumeration at once, also between the
 * two matches of one output. */
static void enum_early_stop(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	EXPECT_ENUM("RTMP", 1, "proto_dup");
	EXPECT_ENUM("RTMP", 2, "proto_dup", "proto_dup");
	EXPECT_ENUM("RTMP", 3, "proto_dup", "proto_dup", "proto_rtmp_rtmps");
	EXPECT_ENUM("RTMP", 4, "proto_dup", "proto_dup", "proto_rtmp_rtmps", "proto_empty_segments");
	EXPECT_ENUM("RTMPS", 1, "proto_rtmp_rtmps");
}

/* What each output registered, through its id and through an instance. */
static void output_protocols_and_flags(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	for (size_t i = 0; i < NUM_OUTPUTS; i++) {
		const struct obs_output_info *info = &outputs[i];
		assert_int_equal(obs_get_output_flags(info->id), info->flags);

		obs_output_t *output = obs_output_create(info->id, info->id, NULL, NULL);
		assert_non_null(output);
		const char *protocols = obs_output_get_protocols(output);
		if (info->flags & OBS_OUTPUT_SERVICE) {
			/* the registered string, as it is */
			assert_non_null(protocols);
			assert_string_equal(protocols, info->protocols);
		} else {
			assert_null(protocols);
		}
		obs_output_release(output);
	}
	assert_int_equal(obs_get_output_flags("proto_not_registered"), 0);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (obs_up) {
		for (size_t i = 0; i < NUM_OUTPUTS; i++)
			obs_register_output(&outputs[i]);
	}
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(registered_protocol_list),   cmocka_unit_test(is_registered),
		cmocka_unit_test(enum_order_and_duplicates),  cmocka_unit_test(enum_whole_names),
		cmocka_unit_test(enum_unregistered),          cmocka_unit_test(enum_early_stop),
		cmocka_unit_test(output_protocols_and_flags),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

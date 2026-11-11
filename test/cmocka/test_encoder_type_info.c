/* Encoder-type metadata characterization before the Rust swap (Refs #4683).
 * Calls actual libobs exports with public registration, not copied bodies.
 * NULL IDs are valid only with an initialized, empty encoder registry:
 * find_encoder otherwise calls strcmp with NULL. Queries without a live core
 * and absent get_name callbacks are outside the valid domain. These limits,
 * empty IDs and raw codec strings are characterized, not endorsed.
 * No encoder instances or graphics/audio pipelines are created. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include <obs-module.h>

static unsigned name_calls;
static void *last_type_data;
static const char *next_name;

static const char *record_name(void *type_data)
{
	++name_calls;
	last_type_data = type_data;
	return next_name;
}

static const char *other_name(void *type_data)
{
	(void)type_data;
	assert_true(false); /* Rejected duplicates must never receive this call. */
	return NULL;
}

static void *unused_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	assert_true(false);
	return NULL;
}

static void unused_destroy(void *data)
{
	(void)data;
	assert_true(false);
}

static bool unused_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	(void)received;
	assert_true(false);
	return false;
}

static size_t unused_frame_size(void *data)
{
	(void)data;
	assert_true(false);
	return 0;
}

static void register_type(struct obs_encoder_info info)
{
	if (!info.get_name)
		info.get_name = record_name;
	info.create = unused_create;
	info.destroy = unused_destroy;
	info.encode = unused_encode;
	info.get_frame_size = unused_frame_size;
	obs_register_encoder(&info);
}

static int setup(void **state)
{
	(void)state;
	name_calls = 0;
	last_type_data = NULL;
	next_name = NULL;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	/* Real shutdown frees the registry. IDs, codecs and type_data have static
	 * lifetime and no free_type_data callback; no encoder was instantiated. */
	obs_shutdown();
	return 0;
}

static void expect_codec(const char *id, const char *codec)
{
	unsigned before = name_calls;
	assert_ptr_equal(obs_get_encoder_codec(id), codec);
	assert_int_equal(name_calls, before);
}

static void expect_missing(const char *id)
{
	unsigned before = name_calls;
	expect_codec(id, NULL);
	assert_null(obs_encoder_get_display_name(id));
	assert_int_equal(name_calls, before);
}

static void missing_and_empty_registry_null_ids(void **state)
{
	(void)state;
	const char *id = NULL;
	assert_false(obs_enum_encoder_types(0, &id));
	expect_missing(NULL); /* No strcmp occurs while encoder_types.num is zero. */
	expect_missing("");
	expect_missing("unknown-encoder-type");
	const struct obs_encoder_info info = {.id = "known-encoder", .type = OBS_ENCODER_VIDEO, .codec = "h264"};
	register_type(info);
	assert_true(obs_enum_encoder_types(0, &id));
	assert_string_equal(id, info.id);
	expect_missing("");
	expect_missing("unknown-encoder-type");
	expect_missing("Known-encoder");
	expect_missing("known-encoder-suffix");
	expect_missing("known-encode");
	/* NULL with a populated registry would be undefined behavior. */
}

static void name_callback_argument_and_result(void **state)
{
	(void)state;
	static unsigned token;
	static char first[] = "same display name";
	static char second[] = "same display name";
	static char empty[] = "";
	static const char bytes[] = {'n', (char)0xff, '\0', 'x', '\0'};
	const struct obs_encoder_info info = {
		.id = "callback-encoder",
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = record_name,
		.type_data = &token,
	};
	register_type(info);
	const char *results[] = {first, second, empty, bytes, NULL, first};
	assert_ptr_not_equal(first, second);
	for (size_t i = 0; i < sizeof(results) / sizeof(results[0]); ++i) {
		next_name = results[i];
		unsigned before = name_calls;
		assert_ptr_equal(obs_encoder_get_display_name(info.id), results[i]);
		assert_int_equal(name_calls, before + 1);
		assert_ptr_equal(last_type_data, &token);
		expect_codec(info.id, info.codec);
	}
	first[0] = 'S';
	assert_int_equal(obs_encoder_get_display_name(info.id)[0], 'S');
	assert_int_equal(name_calls, 7);
	assert_ptr_equal(last_type_data, &token);
	first[0] = 's';
	struct obs_encoder_info null_data = info;
	null_data.id = "callback-null-data";
	null_data.type = OBS_ENCODER_AUDIO;
	null_data.type_data = NULL;
	register_type(null_data);
	next_name = second;
	assert_ptr_equal(obs_encoder_get_display_name(null_data.id), second);
	assert_int_equal(name_calls, 8);
	assert_null(last_type_data);
}

static void exact_borrowed_codec_strings(void **state)
{
	(void)state;
	static char video[] = " H264;;hevc;H264;unknown ";
	static char audio[] = "aac;opus;;aac;";
	static char empty[] = "";
	static const char bytes[] = {'v', (char)0xfe, ';', '\0', 'x', '\0'};
	static char same_video[] = " H264;;hevc;H264;unknown ";
	const char *codecs[] = {NULL, empty, bytes, video, audio, same_video};
	static char ids[6][32];
	assert_ptr_not_equal(video, same_video);
	for (size_t i = 0; i < sizeof(codecs) / sizeof(codecs[0]); ++i) {
		snprintf(ids[i], sizeof(ids[i]), "metadata-codec-%u", (unsigned)i);
		const struct obs_encoder_info info = {
			.id = ids[i],
			.type = i % 2 ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO,
			.codec = codecs[i],
		};
		register_type(info);
		expect_codec(info.id, codecs[i]);
		/* A NULL codec is a matched entry, not a missing type. */
		next_name = video;
		assert_ptr_equal(obs_encoder_get_display_name(info.id), video);
		assert_int_equal(name_calls, i + 1);
		assert_null(last_type_data);
	}
	assert_string_equal(obs_get_encoder_codec(ids[3]), " H264;;hevc;H264;unknown ");
	assert_string_equal(obs_get_encoder_codec(ids[4]), "aac;opus;;aac;");
	video[0] = 'x';
	audio[0] = 'y';
	expect_codec(ids[3], video);
	expect_codec(ids[4], audio);
	assert_int_equal(obs_get_encoder_codec(ids[3])[0], 'x');
	assert_int_equal(obs_get_encoder_codec(ids[4])[0], 'y');
	video[0] = ' ';
	audio[0] = 'a';
}

static void first_match_and_byte_exact_ids(void **state)
{
	(void)state;
	static unsigned first_token, second_token;
	static char first_codec[] = "h264";
	static char second_codec[] = "h264";
	static char result[] = "first registered match";
	static const char raw_id[] = {'e', (char)0xff, '\0'};
	const struct obs_encoder_info entries[] = {
		{.id = "prefix", .type = OBS_ENCODER_VIDEO, .codec = "prefix-codec"},
		{.id = "duplicate",
		 .type = OBS_ENCODER_VIDEO,
		 .codec = first_codec,
		 .get_name = record_name,
		 .type_data = &first_token},
		{.id = "duplicate",
		 .type = OBS_ENCODER_AUDIO,
		 .codec = second_codec,
		 .get_name = other_name,
		 .type_data = &second_token},
		{.id = "duplicate-suffix", .type = OBS_ENCODER_VIDEO, .codec = "suffix-codec"},
		{.id = raw_id, .type = OBS_ENCODER_VIDEO, .codec = "raw-id-codec"},
		{.id = "", .type = OBS_ENCODER_AUDIO, .codec = "empty-id-codec"},
	};
	for (size_t i = 0; i < sizeof(entries) / sizeof(entries[0]); ++i)
		register_type(entries[i]);
	char equal_id[] = "duplicate\0ignored bytes";
	next_name = result;
	assert_ptr_not_equal(first_codec, second_codec);
	for (unsigned i = 0; i < 3; ++i) {
		expect_codec(equal_id, first_codec);
		assert_ptr_equal(obs_encoder_get_display_name(equal_id), result);
		assert_ptr_equal(last_type_data, &first_token);
		assert_int_equal(name_calls, i + 1);
	}
	expect_codec("prefix", entries[0].codec);
	expect_codec("duplicate-suffix", entries[3].codec);
	char equal_raw_id[] = {'e', (char)0xff, '\0', 'x', '\0'};
	expect_codec(equal_raw_id, entries[4].codec);
	assert_ptr_equal(obs_encoder_get_display_name(equal_raw_id), result);
	assert_null(last_type_data);
	expect_codec("", entries[5].codec);
	assert_ptr_equal(obs_encoder_get_display_name(""), result);
	assert_int_equal(name_calls, 5);
	expect_missing("Duplicate");
	expect_missing("duplicat");
	expect_missing("e");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(missing_and_empty_registry_null_ids, setup, teardown),
		cmocka_unit_test_setup_teardown(name_callback_argument_and_result, setup, teardown),
		cmocka_unit_test_setup_teardown(exact_borrowed_codec_strings, setup, teardown),
		cmocka_unit_test_setup_teardown(first_match_and_byte_exact_ids, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

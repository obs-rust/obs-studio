/* Output-type metadata characterization before the Rust swap (Refs #4677).
 * Calls the actual libobs exports, never a compiled copy of their bodies.
 * Fixtures use public registration, including rejected duplicate IDs. Unknown
 * flag bits and raw codec strings are characterized, not endorsed.
 * No output instances or graphics/audio pipelines are created.
 * Non-NULL IDs require a live core; get_name must be callable for a match. */
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
	assert_true(false); /* A later duplicate must never receive this call. */
	return NULL;
}

static void *unused_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	(void)output;
	assert_true(false);
	return NULL;
}

static void unused_destroy(void *data)
{
	(void)data;
	assert_true(false);
}

static bool unused_start(void *data)
{
	(void)data;
	assert_true(false);
	return false;
}

static void unused_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
	assert_true(false);
}

static void unused_video(void *data, struct video_data *frame)
{
	(void)data;
	(void)frame;
	assert_true(false);
}

static void unused_audio(void *data, struct audio_data *frame)
{
	(void)data;
	(void)frame;
	assert_true(false);
}

static void unused_audio2(void *data, size_t idx, struct audio_data *frame)
{
	(void)data;
	(void)idx;
	(void)frame;
	assert_true(false);
}

static void unused_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
	assert_true(false);
}

static void register_type(struct obs_output_info info)
{
	if (!info.get_name)
		info.get_name = record_name;
	info.create = unused_create;
	info.destroy = unused_destroy;
	info.start = unused_start;
	info.stop = unused_stop;
	info.raw_video = unused_video;
	info.raw_audio = unused_audio;
	info.raw_audio2 = unused_audio2;
	info.encoded_packet = unused_packet;
	info.protocols = "output-type-info-fixture";
	obs_register_output(&info);
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
	/* The real shutdown frees the registry darray. Fixtures have no owned
	 * type_data or free_type_data callbacks, exactly as shutdown expects. */
	obs_shutdown();
	return 0;
}

static void expect_metadata(const char *id, uint32_t flags, const char *video, const char *audio)
{
	unsigned before = name_calls;
	assert_int_equal(obs_get_output_flags(id), flags);
	assert_ptr_equal(obs_get_output_supported_video_codecs(id), video);
	assert_ptr_equal(obs_get_output_supported_audio_codecs(id), audio);
	assert_int_equal(name_calls, before);
}

static void expect_missing(const char *id)
{
	unsigned before = name_calls;
	expect_metadata(id, 0, NULL, NULL);
	assert_null(obs_output_get_display_name(id));
	assert_int_equal(name_calls, before);
}

static void missing_and_null_ids(void **state)
{
	(void)state;
	const char *id = NULL;
	assert_false(obs_enum_output_types(0, &id));
	expect_missing(NULL);
	expect_missing("");
	expect_missing("unknown-output-type");
	const struct obs_output_info info = {.id = "known-output", .get_name = record_name, .flags = UINT32_MAX};
	register_type(info);
	expect_missing(NULL);
	expect_missing("");
	expect_missing("unknown-output-type");
	expect_missing("Known-output");
	expect_missing("known-output-suffix");
	expect_missing("known-outpu");
}

static void name_callback_argument_and_result(void **state)
{
	(void)state;
	static unsigned token;
	static char first[] = "same display name";
	static char second[] = "same display name";
	static char empty[] = "";
	static const char bytes[] = {'n', (char)0xff, '\0', 'x', '\0'};
	const struct obs_output_info info = {
		.id = "callback-output",
		.get_name = record_name,
		.type_data = &token,
	};
	register_type(info);
	const char *results[] = {first, second, empty, bytes, NULL, first};
	assert_ptr_not_equal(first, second);
	for (size_t i = 0; i < sizeof(results) / sizeof(results[0]); ++i) {
		next_name = results[i];
		unsigned before = name_calls;
		assert_ptr_equal(obs_output_get_display_name(info.id), results[i]);
		assert_int_equal(name_calls, before + 1);
		assert_ptr_equal(last_type_data, &token);
	}
	/* A registered callback receives NULL type_data unchanged too. */
	struct obs_output_info null_data = info;
	null_data.id = "callback-null-data";
	null_data.type_data = NULL;
	register_type(null_data);
	next_name = second;
	assert_ptr_equal(obs_output_get_display_name(null_data.id), second);
	assert_int_equal(name_calls, 7);
	assert_null(last_type_data);
	expect_metadata(info.id, 0, NULL, NULL);
}

static void exact_flags_and_borrowed_codec_strings(void **state)
{
	(void)state;
	static char video[] = " H264;;hevc;H264;unknown ";
	static char audio[] = "aac;opus;;aac;";
	static char empty[] = "";
	static const char bytes[] = {'v', (char)0xfe, ';', '\0', 'x', '\0'};
	const struct obs_output_info info = {
		.id = "metadata-output",
		.encoded_video_codecs = video,
		.encoded_audio_codecs = audio,
	};
	/* Metadata queries must not invoke the registered name callback. */
	register_type(info);
	static char flag_ids[32][32];
	for (unsigned bit = 0; bit < 32; ++bit) {
		uint32_t flags = UINT32_C(1) << bit;
		snprintf(flag_ids[bit], sizeof(flag_ids[bit]), "metadata-bit-%u", bit);
		struct obs_output_info entry = info;
		entry.id = flag_ids[bit];
		entry.flags = flags;
		register_type(entry);
		expect_metadata(entry.id, flags, video, audio);
	}
	struct obs_output_info all_flags = info;
	all_flags.id = "metadata-all-flags";
	all_flags.flags = UINT32_MAX;
	register_type(all_flags);
	expect_metadata(all_flags.id, UINT32_MAX, video, audio);
	assert_string_equal(video, " H264;;hevc;H264;unknown ");
	assert_string_equal(audio, "aac;opus;;aac;");
	video[0] = 'x';
	audio[0] = 'y';
	expect_metadata(info.id, 0, video, audio);
	assert_int_equal(obs_get_output_supported_video_codecs(info.id)[0], 'x');
	assert_int_equal(obs_get_output_supported_audio_codecs(info.id)[0], 'y');
	video[0] = ' ';
	audio[0] = 'a';
	const char *codecs[] = {NULL, empty, bytes, video, audio};
	static char codec_ids[5][32];
	for (size_t i = 0; i < sizeof(codecs) / sizeof(codecs[0]); ++i) {
		snprintf(codec_ids[i], sizeof(codec_ids[i]), "metadata-codecs-%u", (unsigned)i);
		struct obs_output_info entry = info;
		entry.id = codec_ids[i];
		entry.encoded_video_codecs = codecs[i];
		entry.encoded_audio_codecs = codecs[4 - i];
		register_type(entry);
		expect_metadata(entry.id, 0, codecs[i], codecs[4 - i]);
	}
}

static void first_match_and_byte_exact_ids(void **state)
{
	(void)state;
	static unsigned first_token, second_token;
	static char first_video[] = "h264";
	static char second_video[] = "h264";
	static char first_audio[] = "aac";
	static char second_audio[] = "aac";
	static char result[] = "first registered match";
	static const char raw_id[] = {'o', (char)0xff, '\0'};
	const struct obs_output_info entries[] = {
		{.id = "prefix", .flags = 1},
		{.id = "duplicate",
		 .flags = UINT32_C(0x80000045),
		 .get_name = record_name,
		 .type_data = &first_token,
		 .encoded_video_codecs = first_video,
		 .encoded_audio_codecs = first_audio},
		{.id = "duplicate",
		 .flags = 2,
		 .get_name = other_name,
		 .type_data = &second_token,
		 .encoded_video_codecs = second_video,
		 .encoded_audio_codecs = second_audio},
		{.id = "duplicate-suffix", .flags = 4},
		{.id = raw_id, .flags = 8, .get_name = record_name},
		{.id = "", .flags = 16, .get_name = record_name},
	};
	for (size_t i = 0; i < sizeof(entries) / sizeof(entries[0]); ++i)
		register_type(entries[i]);
	char equal_id[] = "duplicate\0ignored bytes";
	next_name = result;
	for (unsigned i = 0; i < 3; ++i) {
		expect_metadata(equal_id, entries[1].flags, first_video, first_audio);
		assert_ptr_equal(obs_output_get_display_name(equal_id), result);
		assert_ptr_equal(last_type_data, &first_token);
		assert_int_equal(name_calls, i + 1);
	}
	assert_ptr_not_equal(first_video, second_video);
	assert_ptr_not_equal(first_audio, second_audio);
	expect_metadata("prefix", 1, NULL, NULL);
	expect_metadata("duplicate-suffix", 4, NULL, NULL);
	char equal_raw_id[] = {'o', (char)0xff, '\0', 'x', '\0'};
	expect_metadata(equal_raw_id, 8, NULL, NULL);
	assert_ptr_equal(obs_output_get_display_name(equal_raw_id), result);
	assert_null(last_type_data);
	expect_metadata("", 16, NULL, NULL);
	assert_ptr_equal(obs_output_get_display_name(""), result);
	assert_int_equal(name_calls, 5);
	expect_missing("Duplicate");
	expect_missing("duplicat");
	expect_missing("o");
	expect_missing(NULL);
}

static void null_ids_without_core(void **state)
{
	(void)state;
	assert_false(obs_initialized());
	expect_missing(NULL); /* find_output returns before touching obs. */
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_ids_without_core),
		cmocka_unit_test_setup_teardown(missing_and_null_ids, setup, teardown),
		cmocka_unit_test_setup_teardown(name_callback_argument_and_result, setup, teardown),
		cmocka_unit_test_setup_teardown(exact_flags_and_borrowed_codec_strings, setup, teardown),
		cmocka_unit_test_setup_teardown(first_match_and_byte_exact_ids, setup, teardown),
		cmocka_unit_test(null_ids_without_core),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

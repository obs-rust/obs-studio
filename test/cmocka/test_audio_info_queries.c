/* Characterize obs_get_audio_info and obs_get_audio_info2 before the Rust
 * swap (Refs #5169): the exact extracted bodies run against a stack core, with
 * audio_output_get_info and the buffering-milliseconds helper replaced by
 * probes that record every call. No OBS startup or audio thread is needed.
 * The milliseconds arithmetic itself is pinned by test_core_setup_rules_ms_steps;
 * here only its arguments, its call order and the uint32_t cast are pinned. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

#if !__has_include("libobs/obs-audio-info-queries.c.h")
#error "audio-info query extraction seam is missing"
#endif

/* The bodies under test read this core, not the library's hidden global. */
struct obs_core *obs;

const struct audio_output_info *probe_get_info(const audio_t *audio);
int probe_buffer_ms(int ticks, uint32_t samples_per_sec);
#define obs_get_audio_info info_body
#define obs_get_audio_info2 info2_body
#define audio_output_get_info probe_get_info
#define obs_core_setup_rules_buffer_ms probe_buffer_ms
#include "libobs/obs-audio-info-queries.c.h"
#undef obs_core_setup_rules_buffer_ms
#undef audio_output_get_info
#undef obs_get_audio_info2
#undef obs_get_audio_info

static struct obs_core core;
static struct audio_output_info output_info;

/* A non-NULL handle the probe compares against; never dereferenced. */
static char fake_audio_storage;
#define FAKE_AUDIO ((audio_t *)&fake_audio_storage)

/* Call log: 'i' for audio_output_get_info, 'm' for the milliseconds helper. */
static char calls[8];
static size_t num_calls;
static int ms_ticks;
static uint32_t ms_rate;
static int ms_result;

const struct audio_output_info *probe_get_info(const audio_t *audio)
{
	assert_ptr_equal(audio, FAKE_AUDIO);
	assert_true(num_calls < sizeof(calls));
	calls[num_calls++] = 'i';
	return &output_info;
}

int probe_buffer_ms(int ticks, uint32_t samples_per_sec)
{
	assert_true(num_calls < sizeof(calls));
	calls[num_calls++] = 'm';
	ms_ticks = ticks;
	ms_rate = samples_per_sec;
	return ms_result;
}

static int setup(void **state)
{
	(void)state;
	memset(&core, 0, sizeof(core));
	memset(&output_info, 0, sizeof(output_info));
	memset(calls, 0, sizeof(calls));
	num_calls = 0;
	ms_ticks = -1;
	ms_rate = 0;
	ms_result = 0;
	core.audio.audio = FAKE_AUDIO;
	obs = &core;
	return 0;
}

static void assert_calls(const char *expected)
{
	assert_int_equal(num_calls, strlen(expected));
	assert_memory_equal(calls, expected, num_calls);
}

#define SENTINEL_BYTE 0xA5

static void fill_sentinel(void *p, size_t size)
{
	memset(p, SENTINEL_BYTE, size);
}

static void assert_sentinel(const void *p, size_t size)
{
	const unsigned char *bytes = p;
	for (size_t i = 0; i < size; i++)
		assert_int_equal(bytes[i], SENTINEL_BYTE);
}

/* No core: both reject before forming &obs->audio and write nothing (#3363). */
static void test_no_core(void **state)
{
	(void)state;
	struct obs_audio_info oai;
	struct obs_audio_info2 oai2;
	obs = NULL;

	fill_sentinel(&oai, sizeof(oai));
	assert_false(info_body(&oai));
	assert_sentinel(&oai, sizeof(oai));

	fill_sentinel(&oai2, sizeof(oai2));
	assert_false(info2_body(&oai2));
	assert_sentinel(&oai2, sizeof(oai2));

	assert_false(info_body(NULL));
	assert_false(info2_body(NULL));
	assert_calls("");
}

static void test_info_null_output(void **state)
{
	(void)state;
	assert_false(info_body(NULL));
	assert_calls("");
}

static void test_info_no_audio(void **state)
{
	(void)state;
	struct obs_audio_info oai;
	core.audio.audio = NULL;
	fill_sentinel(&oai, sizeof(oai));
	assert_false(info_body(&oai));
	assert_sentinel(&oai, sizeof(oai));
	assert_calls("");
}

/* Success: one info read, rate and speakers copied, nothing else read. */
static void test_info_copies_rate_and_speakers(void **state)
{
	(void)state;
	static const struct {
		uint32_t rate;
		enum speaker_layout speakers;
	} cases[] = {
		{44100, SPEAKERS_STEREO},
		{48000, SPEAKERS_7POINT1},
		{0, SPEAKERS_UNKNOWN},
		{UINT32_MAX, SPEAKERS_MONO},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		struct obs_audio_info oai;
		num_calls = 0;
		output_info.samples_per_sec = cases[i].rate;
		output_info.speakers = cases[i].speakers;
		output_info.format = AUDIO_FORMAT_FLOAT_PLANAR;
		fill_sentinel(&oai, sizeof(oai));
		assert_true(info_body(&oai));
		assert_int_equal(oai.samples_per_sec, cases[i].rate);
		assert_int_equal(oai.speakers, cases[i].speakers);
		assert_calls("i");
	}
}

/* info2 calls obs_get_audio_info into its local first, even when the extended
 * output is NULL; only then does it check oai2. */
static void test_info2_null_output_still_reads_info(void **state)
{
	(void)state;
	output_info.samples_per_sec = 48000;
	output_info.speakers = SPEAKERS_STEREO;
	assert_false(info2_body(NULL));
	assert_calls("i");
}

static void test_info2_no_audio(void **state)
{
	(void)state;
	struct obs_audio_info2 oai2;
	core.audio.audio = NULL;
	fill_sentinel(&oai2, sizeof(oai2));
	assert_false(info2_body(&oai2));
	assert_sentinel(&oai2, sizeof(oai2));
	assert_calls("");
}

/* Success: rate and speakers from the info read, fixed_buffering from the core,
 * then the milliseconds helper with the core's ticks and the copied rate. */
static void test_info2_copies_all_fields(void **state)
{
	(void)state;
	static const struct {
		uint32_t rate;
		enum speaker_layout speakers;
		bool fixed;
		int ticks;
		int ms;
	} cases[] = {
		{48000, SPEAKERS_STEREO, false, 1, 21},
		{44100, SPEAKERS_5POINT1, true, 45, 1000},
		{96000, SPEAKERS_4POINT0, true, 0, 0},
		{8000, SPEAKERS_MONO, false, INT32_MAX, INT32_MAX},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		struct obs_audio_info2 oai2;
		num_calls = 0;
		output_info.samples_per_sec = cases[i].rate;
		output_info.speakers = cases[i].speakers;
		core.audio.fixed_buffer = cases[i].fixed;
		core.audio.max_buffering_ticks = cases[i].ticks;
		ms_result = cases[i].ms;
		fill_sentinel(&oai2, sizeof(oai2));

		assert_true(info2_body(&oai2));
		assert_int_equal(oai2.samples_per_sec, cases[i].rate);
		assert_int_equal(oai2.speakers, cases[i].speakers);
		assert_int_equal(oai2.fixed_buffering, cases[i].fixed);
		assert_int_equal(oai2.max_buffering_ms, (uint32_t)cases[i].ms);
		assert_int_equal(ms_ticks, cases[i].ticks);
		assert_int_equal(ms_rate, cases[i].rate);
		assert_calls("im");
	}
}

/* The helper's int result is cast straight to uint32_t, negative included. */
static void test_info2_casts_helper_result(void **state)
{
	(void)state;
	struct obs_audio_info2 oai2;
	output_info.samples_per_sec = 48000;
	output_info.speakers = SPEAKERS_STEREO;
	ms_result = -1;
	assert_true(info2_body(&oai2));
	assert_int_equal(oai2.max_buffering_ms, UINT32_MAX);
	assert_calls("im");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(test_no_core, setup),
		cmocka_unit_test_setup(test_info_null_output, setup),
		cmocka_unit_test_setup(test_info_no_audio, setup),
		cmocka_unit_test_setup(test_info_copies_rate_and_speakers, setup),
		cmocka_unit_test_setup(test_info2_null_output_still_reads_info, setup),
		cmocka_unit_test_setup(test_info2_no_audio, setup),
		cmocka_unit_test_setup(test_info2_copies_all_fields, setup),
		cmocka_unit_test_setup(test_info2_casts_helper_result, setup),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

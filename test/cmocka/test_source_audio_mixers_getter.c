/* Characterize the public getter before its later Rust swap (Refs #5060). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "obs-internal.h"

#if !__has_include("obs-source-audio-mixers-getter.c.h")
#error "source audio mixers getter extraction seam is missing"
#endif

static unsigned log_count;
static int log_level;
static char message[256];

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(message, sizeof(message), format, args);
}

static void null_source(void **state)
{
	(void)state;
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	log_count = 0;
	base_set_log_handler(capture, NULL);
	const uint32_t mixers = obs_source_get_audio_mixers(NULL);
	base_set_log_handler(previous, param);
	assert_int_equal(mixers, 0);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(message, "obs_source_get_audio_mixers: Null 'source' parameter");
}

static void capability_gate(void **state)
{
	(void)state;
	/* No allocations or owned state: this getter only reads these fields. */
	struct obs_source source = {0};
	source.audio_mixers = UINT32_MAX;
	const uint32_t flags[] = {0, OBS_SOURCE_VIDEO, OBS_SOURCE_COMPOSITE, OBS_SOURCE_VIDEO | OBS_SOURCE_ASYNC};
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	log_count = 0;
	base_set_log_handler(capture, NULL);
	uint32_t gated[sizeof(flags) / sizeof(flags[0])];
	for (size_t i = 0; i < sizeof(flags) / sizeof(flags[0]); i++) {
		source.info.output_flags = flags[i];
		gated[i] = obs_source_get_audio_mixers(&source);
	}
	base_set_log_handler(previous, param);
	for (size_t i = 0; i < sizeof(flags) / sizeof(flags[0]); i++)
		assert_int_equal(gated[i], 0);
	assert_int_equal(log_count, 0);

	source.info.output_flags = OBS_SOURCE_AUDIO;
	assert_int_equal(obs_source_get_audio_mixers(&source), UINT32_MAX);
	/* Missing source types bypass capability gating: characterized, not endorsed. */
	source.info.output_flags = 0;
	source.owns_info_id = true;
	assert_int_equal(obs_source_get_audio_mixers(&source), UINT32_MAX);
}

static const char *source_name(void *unused)
{
	(void)unused;
	return "audio mixers getter probe";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void source_destroy(void *data)
{
	(void)data;
}

static struct obs_source_info audio_info = {
	.id = "test_audio_mixers_getter_audio",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = source_name,
	.create = source_create,
	.destroy = source_destroy,
};

static struct obs_source_info silent_info = {
	.id = "test_audio_mixers_getter_silent",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = 0,
	.get_name = source_name,
	.create = source_create,
	.destroy = source_destroy,
};

static void setter_round_trip(void **state)
{
	(void)state;
	obs_source_t *audio = obs_source_create_private(audio_info.id, "audio", NULL);
	obs_source_t *silent = obs_source_create_private(silent_info.id, "silent", NULL);
	assert_non_null(audio);
	assert_non_null(silent);
	assert_int_equal(obs_source_get_audio_mixers(audio), 0xFF);
	assert_int_equal(obs_source_get_audio_mixers(silent), 0);
	const uint32_t masks[] = {0, 1, 0x3F, UINT32_C(0x80000000), UINT32_MAX, UINT32_C(0xA5A55A5A), 0};
	for (size_t i = 0; i < sizeof(masks) / sizeof(masks[0]); i++) {
		obs_source_set_audio_mixers(audio, masks[i]);
		assert_int_equal(obs_source_get_audio_mixers(audio), masks[i]);
		obs_source_set_audio_mixers(silent, masks[i]);
		assert_int_equal(obs_source_get_audio_mixers(silent), 0);
	}
	obs_source_release(audio);
	obs_source_release(silent);
	while (obs_wait_for_destroy_queue())
		;
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 1;
	obs_register_source(&audio_info);
	obs_register_source(&silent_info);
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),
		cmocka_unit_test(capability_gate),
		cmocka_unit_test(setter_round_trip),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}

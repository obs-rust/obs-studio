/* Public constructor wrappers, characterized against each real libobs backend. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static const char *type_name(void *data)
{
	(void)data;
	return "Constructor characterization";
}

static void *codec_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	/* Construction must remain lazy: no codec instance is created. */
	fail();
	return NULL;
}

static void codec_destroy(void *data)
{
	(void)data;
	fail();
}

static bool codec_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	(void)received;
	fail();
	return false;
}

static size_t frame_size(void *data)
{
	(void)data;
	return 1024;
}

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_encoder_info info = {.id = "constructor-video",
					.type = OBS_ENCODER_VIDEO,
					.codec = "h264",
					.get_name = type_name,
					.create = codec_create,
					.destroy = codec_destroy,
					.encode = codec_encode,
					.get_frame_size = frame_size};
	obs_register_encoder(&info);
	info.id = "constructor-audio";
	info.type = OBS_ENCODER_AUDIO;
	info.codec = "aac";
	obs_register_encoder(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void null_admission(void **state)
{
	(void)state;
	assert_null(obs_video_encoder_create(NULL, "video", NULL, NULL));
	assert_null(obs_video_encoder_create("constructor-video", NULL, NULL, NULL));
	assert_null(obs_video_encoder_create(NULL, NULL, NULL, NULL));
	assert_null(obs_audio_encoder_create(NULL, "audio", NULL, 0, NULL));
	assert_null(obs_audio_encoder_create("constructor-audio", NULL, NULL, 0, NULL));
	assert_null(obs_audio_encoder_create(NULL, NULL, NULL, SIZE_MAX, NULL));
}

static void wrong_type(void **state)
{
	(void)state;
	assert_null(obs_video_encoder_create("constructor-audio", "wrong-video", NULL, NULL));
	assert_null(obs_audio_encoder_create("constructor-video", "wrong-audio", NULL, 3, NULL));
	assert_null(obs_get_encoder_by_name("wrong-video"));
	assert_null(obs_get_encoder_by_name("wrong-audio"));
}

static void check_created(const char *id, bool audio, size_t mixer, bool supplied, bool unknown)
{
	obs_data_t *settings = supplied ? obs_data_create() : NULL;
	obs_data_t *hotkeys = supplied ? obs_data_create() : NULL;
	if (supplied) {
		obs_data_set_int(settings, "marker", 73);
		obs_data_set_string(hotkeys, "marker", "hotkeys");
	}
	char name[] = "constructor-instance";
	obs_encoder_t *encoder = audio ? obs_audio_encoder_create(id, name, settings, mixer, hotkeys)
				       : obs_video_encoder_create(id, name, settings, hotkeys);
	assert_non_null(encoder);
	assert_int_equal(encoder->info.type, audio ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO);
	assert_string_equal(encoder->info.id, id);
	assert_string_equal(encoder->orig_info.id, id);
	assert_int_equal(encoder->owns_info_id, unknown);
	assert_true(encoder->mixer_idx == (audio ? mixer : 0));
	assert_int_equal(encoder->frame_rate_divisor, audio ? 0 : 1);
	assert_null(encoder->context.data);
	assert_false(encoder->active);
	assert_non_null(encoder->context.control);
	if (supplied) {
		assert_ptr_equal(encoder->context.settings, settings);
		assert_ptr_equal(encoder->context.hotkey_data, hotkeys);
	}
	obs_data_release(settings);
	obs_data_release(hotkeys);
	assert_int_equal(obs_data_get_int(encoder->context.settings, "marker"), supplied ? 73 : 0);
	assert_string_equal(obs_data_get_string(encoder->context.hotkey_data, "marker"), supplied ? "hotkeys" : "");
	name[0] = 'X';
	assert_string_equal(obs_encoder_get_name(encoder), "constructor-instance");
	obs_encoder_t *found = obs_get_encoder_by_name("constructor-instance");
	assert_ptr_equal(found, encoder);
	obs_encoder_release(found);
	obs_encoder_release(encoder);
	assert_null(obs_get_encoder_by_name("constructor-instance"));
}

static void registered_creation(void **state)
{
	(void)state;
	const size_t mixers[] = {0, 3, MAX_AUDIO_MIXES - 1, MAX_AUDIO_MIXES, SIZE_MAX};
	for (unsigned supplied = 0; supplied < 2; supplied++) {
		check_created("constructor-video", false, 0, supplied, false);
		for (size_t i = 0; i < sizeof(mixers) / sizeof(mixers[0]); i++)
			/* Out-of-range mixer storage is characterized, not endorsed. */
			check_created("constructor-audio", true, mixers[i], supplied, false);
	}
}

static void unknown_creation(void **state)
{
	(void)state;
	for (unsigned supplied = 0; supplied < 2; supplied++) {
		/* Unknown IDs produce usable placeholder handles, not NULL. */
		check_created("constructor-missing-video", false, 0, supplied, true);
		check_created("constructor-missing-audio", true, SIZE_MAX, supplied, true);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_admission), cmocka_unit_test(wrong_type),
					   cmocka_unit_test(registered_creation), cmocka_unit_test(unknown_creation)};
	return cmocka_run_group_tests(tests, setup, teardown);
}

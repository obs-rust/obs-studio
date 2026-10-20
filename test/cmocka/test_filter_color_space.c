/* Regression test for upstream obsproject/obs-studio PR #13761: an
 * audio-only filter (Gain, Noise Suppression, ...) in a source's filter
 * chain answered obs_source_get_color_space() itself, with sRGB, instead of
 * passing the query on to the filter or source below it. A filter above it
 * that asks its target for the colour space (HDR Tone Mapping, for one) then
 * took an HDR source for SDR and did nothing, depending only on the order
 * the filters were added in.
 *
 * The test registers an HDR (Rec. 2100 PQ) source with audio, an audio-only
 * filter, a video filter with no colour space of its own and a tone-mapping
 * filter that, like hdr-tonemap-filter, asks its target for the colour space.
 * It needs no video or graphics. obs_startup() needs a display on Linux (run
 * under xvfb-run); the cases skip when it cannot start. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <obs.h>
#include <util/base.h>

#define HDR_ID "color_space_hdr_source"
#define AUDIO_FILTER_ID "color_space_audio_filter"
#define VIDEO_FILTER_ID "color_space_video_filter"
#define TONEMAP_ID "color_space_tonemap_filter"

static bool obs_up;

/* What the tone-mapping filter last saw as its input's colour space. */
static enum gs_color_space tonemap_input = GS_CS_SRGB;

static void log_handler(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (lvl > LOG_WARNING)
		return;
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static const char *type_name(void *type_data)
{
	(void)type_data;
	return "color space test";
}

static void *data_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void data_destroy(void *data)
{
	(void)data;
}

static uint32_t hdr_size(void *data)
{
	(void)data;
	return 64;
}

static enum gs_color_space hdr_color_space(void *data, size_t count, const enum gs_color_space *preferred_spaces)
{
	(void)data;
	(void)count;
	(void)preferred_spaces;
	return GS_CS_709_EXTENDED;
}

static struct obs_audio_data *audio_filter_audio(void *data, struct obs_audio_data *audio)
{
	(void)data;
	return audio;
}

static void video_filter_render(void *data, gs_effect_t *effect)
{
	(void)effect;
	obs_source_skip_video_filter(data);
}

/* Like hdr_tonemap_filter_get_color_space(): asks the filter's target. */
static enum gs_color_space tonemap_color_space(void *data, size_t count, const enum gs_color_space *preferred_spaces)
{
	const enum gs_color_space potential_spaces[] = {GS_CS_SRGB, GS_CS_SRGB_16F, GS_CS_709_EXTENDED};
	(void)count;
	(void)preferred_spaces;

	tonemap_input = obs_source_get_color_space(obs_filter_get_target(data), OBS_COUNTOF(potential_spaces),
						   potential_spaces);
	return tonemap_input == GS_CS_709_EXTENDED ? GS_CS_SRGB : tonemap_input;
}

static void register_types(void)
{
	struct obs_source_info hdr = {
		.id = HDR_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_AUDIO,
		.get_name = type_name,
		.create = data_create,
		.destroy = data_destroy,
		.get_width = hdr_size,
		.get_height = hdr_size,
		.video_get_color_space = hdr_color_space,
	};
	struct obs_source_info audio_filter = {
		.id = AUDIO_FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = type_name,
		.create = data_create,
		.destroy = data_destroy,
		.filter_audio = audio_filter_audio,
	};
	struct obs_source_info video_filter = {
		.id = VIDEO_FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = type_name,
		.create = data_create,
		.destroy = data_destroy,
		.video_render = video_filter_render,
	};
	struct obs_source_info tonemap = {
		.id = TONEMAP_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = type_name,
		.create = data_create,
		.destroy = data_destroy,
		.video_render = video_filter_render,
		.video_get_color_space = tonemap_color_space,
	};
	obs_register_source(&hdr);
	obs_register_source(&audio_filter);
	obs_register_source(&video_filter);
	obs_register_source(&tonemap);
}

static obs_source_t *add_filter(obs_source_t *source, const char *id, const char *name)
{
	obs_source_t *filter = obs_source_create(id, name, NULL, NULL);
	assert_non_null(filter);
	obs_source_filter_add(source, filter);
	obs_source_release(filter);
	return filter;
}

static int setup(void **state)
{
	obs_source_t *source;

	if (!obs_up)
		return 0;

	source = obs_source_create(HDR_ID, "hdr source", NULL, NULL);
	assert_non_null(source);
	tonemap_input = GS_CS_SRGB;
	*state = source;
	return 0;
}

static void no_task(void *param)
{
	(void)param;
}

static int teardown(void **state)
{
	obs_source_t *source = *state;

	if (!source)
		return 0;

	obs_source_remove(source);
	obs_source_release(source);
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	*state = NULL;
	return 0;
}

static const enum gs_color_space all_spaces[] = {GS_CS_SRGB, GS_CS_SRGB_16F, GS_CS_709_EXTENDED};

static enum gs_color_space space_of(obs_source_t *source)
{
	return obs_source_get_color_space(source, OBS_COUNTOF(all_spaces), all_spaces);
}

/* source <- Gain: the audio filter answers for the source below it. */
static void test_audio_filter_passes_query_on(void **state)
{
	obs_source_t *source = *state;
	obs_source_t *gain;

	if (!obs_up)
		skip();

	gain = add_filter(source, AUDIO_FILTER_ID, "gain");
	assert_int_equal(space_of(gain), GS_CS_709_EXTENDED);
}

/* source <- Gain <- Tone Mapping (the order of upstream #13761): the
 * tone-mapping filter sees the HDR input through the audio filter. */
static void test_tonemap_above_audio_filter(void **state)
{
	obs_source_t *source = *state;
	obs_source_t *tonemap;

	if (!obs_up)
		skip();

	add_filter(source, AUDIO_FILTER_ID, "gain");
	tonemap = add_filter(source, TONEMAP_ID, "tonemap");
	assert_int_equal(space_of(tonemap), GS_CS_SRGB);
	assert_int_equal(tonemap_input, GS_CS_709_EXTENDED);
}

/* source <- Tone Mapping <- Gain: the other order. The query from the top
 * reaches the tone-mapping filter. */
static void test_audio_filter_above_tonemap(void **state)
{
	obs_source_t *source = *state;
	obs_source_t *gain;

	if (!obs_up)
		skip();

	add_filter(source, TONEMAP_ID, "tonemap");
	gain = add_filter(source, AUDIO_FILTER_ID, "gain");
	assert_int_equal(space_of(gain), GS_CS_SRGB);
	assert_int_equal(tonemap_input, GS_CS_709_EXTENDED);
}

/* Control: a disabled audio filter already passed the query on. */
static void test_disabled_audio_filter(void **state)
{
	obs_source_t *source = *state;
	obs_source_t *gain;

	if (!obs_up)
		skip();

	gain = add_filter(source, AUDIO_FILTER_ID, "gain");
	obs_source_set_enabled(gain, false);
	assert_int_equal(space_of(gain), GS_CS_709_EXTENDED);
}

/* Control: a video filter with no colour space callback still answers
 * sRGB itself, as before (unchanged by the fix). */
static void test_video_filter_without_callback(void **state)
{
	obs_source_t *source = *state;
	obs_source_t *video;

	if (!obs_up)
		skip();

	video = add_filter(source, VIDEO_FILTER_ID, "video");
	assert_int_equal(space_of(video), GS_CS_SRGB);
}

static int group_setup(void **state)
{
	(void)state;
	base_set_log_handler(log_handler, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (obs_up)
		register_types();
	else
		fprintf(stderr, "obs_startup failed (no display?), skipping\n");
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(test_audio_filter_passes_query_on, setup, teardown),
		cmocka_unit_test_setup_teardown(test_tonemap_above_audio_filter, setup, teardown),
		cmocka_unit_test_setup_teardown(test_audio_filter_above_tonemap, setup, teardown),
		cmocka_unit_test_setup_teardown(test_disabled_audio_filter, setup, teardown),
		cmocka_unit_test_setup_teardown(test_video_filter_without_callback, setup, teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

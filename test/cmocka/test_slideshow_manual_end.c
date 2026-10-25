/* Image Slide Show in Manual mode with Playback Mode "Once": Next on the last
 * slide goes back to the first slide (forum thread 138392: "Image Slide Show
 * Not Stopping at end in Manual Mode", with Loop off and "Hide when
 * slideshow is done" on).
 *
 * In Automatic mode, ss_video_tick() ends a "Once" slideshow on its last
 * slide: it stays there, or fades out and reports the media as ended with
 * "Hide when slideshow is done". ss_next_slide() (the Next hotkey, the
 * media controls' Next button and obs-websocket's NEXT action) always
 * wrapped to the first slide instead.
 *
 * The original slideshow (obs-slideshow.c), which sources saved before the
 * rework still use, has the same Next.
 *
 * The test runs both real slideshows (obs-slideshow-mk2.c, obs-slideshow.c)
 * and the real Cut transition (transition-cut.c), built into the test, with
 * a stub Image source for the slides, as the program output with core video
 * running.
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run).
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <obs-module.h>
#include <util/platform.h>

extern struct obs_source_info slideshow_info;
extern struct obs_source_info slideshow_info_mk2;
extern struct obs_source_info cut_transition;

static bool obs_up = false;

/* ------------------------------------------------------------------------- */
/* what the slideshow uses from the rest of the image-source module */

const char *obs_module_text(const char *lookup)
{
	return lookup;
}

uint64_t image_source_get_memory_usage(void *data)
{
	UNUSED_PARAMETER(data);
	return 0;
}

void image_source_preload_image(void *data)
{
	UNUSED_PARAMETER(data);
}

void image_source_ready_slide(void *data)
{
	UNUSED_PARAMETER(data);
}

static const char *slide_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "test slide";
}

static void *slide_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void slide_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static uint32_t slide_size(void *data)
{
	UNUSED_PARAMETER(data);
	return 16;
}

static void slide_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(effect);
}

static void register_types(void)
{
	struct obs_source_info slide = {
		.id = "image_source",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = slide_name,
		.create = slide_create,
		.destroy = slide_destroy,
		.get_width = slide_size,
		.get_height = slide_size,
		.video_render = slide_render,
	};
	obs_register_source(&slide);
	obs_register_source(&cut_transition);
	obs_register_source(&slideshow_info);
	obs_register_source(&slideshow_info_mk2);
}

/* ------------------------------------------------------------------------- */

#define SLIDES 3

/* kind: "slideshow_v2" (the current slideshow) or "slideshow" (the original
 * one, which has a Loop checkbox instead of a Playback Mode) */
static obs_source_t *create_slideshow(const char *kind, const char *playback_mode, bool hide)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "slide_mode", "mode_manual");
	obs_data_set_string(settings, "playback_mode", playback_mode);
	obs_data_set_bool(settings, "loop", strcmp(playback_mode, "loop") == 0);
	obs_data_set_bool(settings, "hide", hide);
	obs_data_set_string(settings, "transition", "cut");
	obs_data_set_string(settings, "use_custom_size", "64x64");

	obs_data_array_t *files = obs_data_array_create();
	for (int i = 0; i < SLIDES; i++) {
		char path[32];
		snprintf(path, sizeof(path), "slide%d.png", i);
		obs_data_t *item = obs_data_create();
		obs_data_set_string(item, "value", path);
		obs_data_array_push_back(files, item);
		obs_data_release(item);
	}
	obs_data_set_array(settings, "files", files);
	obs_data_array_release(files);

	obs_source_t *ss = obs_source_create(kind, kind, settings, NULL);
	obs_data_release(settings);
	assert_non_null(ss);
	assert_string_equal(obs_source_get_unversioned_id(ss), "slideshow");

	/* on the program output, so it is active and its transition ticks */
	obs_set_output_source(0, ss);
	os_sleep_ms(200);
	return ss;
}

static void destroy_slideshow(obs_source_t *ss)
{
	obs_set_output_source(0, NULL);
	obs_source_release(ss);
}

static long long current_index(obs_source_t *ss)
{
	calldata_t cd = {0};
	proc_handler_call(obs_source_get_proc_handler(ss), "current_index", &cd);
	long long idx = calldata_int(&cd, "current_index");
	calldata_free(&cd);
	return idx;
}

static void press_next(obs_source_t *ss)
{
	obs_source_media_next(ss);
	/* let the Cut transition finish on the video thread */
	os_sleep_ms(150);
}

static void get_transition_cb(obs_source_t *parent, obs_source_t *child, void *param)
{
	UNUSED_PARAMETER(parent);
	obs_source_t **out = param;
	if (obs_source_get_type(child) == OBS_SOURCE_TYPE_TRANSITION)
		*out = child;
}

/* The slideshow's active child is its transition, and the transition's
 * active source is the slide on screen (none once it has faded out). */
static bool slide_on_screen(obs_source_t *ss)
{
	obs_source_t *transition = NULL;
	obs_source_enum_active_sources(ss, get_transition_cb, &transition);
	if (!transition)
		return false;
	obs_source_t *active = obs_transition_get_active_source(transition);
	bool on = active != NULL;
	obs_source_release(active);
	return on;
}

/* "Once", no hide: Next on the last slide keeps the last slide. */
static void once_next_on_last_slide_stays(const char *kind)
{
	obs_source_t *ss = create_slideshow(kind, "once", false);
	assert_int_equal(current_index(ss), 0);

	press_next(ss);
	press_next(ss);
	assert_int_equal(current_index(ss), SLIDES - 1);

	press_next(ss);
	long long idx = current_index(ss);
	bool on = slide_on_screen(ss);
	enum obs_media_state media = obs_source_media_get_state(ss);
	printf("%s once: after Next on the last slide: slide %lld, on screen %d, media state %d\n", kind, idx, on,
	       media);
	destroy_slideshow(ss);

	assert_int_equal(idx, SLIDES - 1);
	assert_true(on);
}

/* "Once" + "Hide when slideshow is done": Next on the last slide ends the
 * slideshow as Automatic mode does: nothing on screen, media state Ended. */
static void once_hide_next_on_last_slide_ends(const char *kind)
{
	obs_source_t *ss = create_slideshow(kind, "once", true);
	press_next(ss);
	press_next(ss);
	assert_int_equal(current_index(ss), SLIDES - 1);
	assert_true(slide_on_screen(ss));

	press_next(ss);
	long long idx = current_index(ss);
	bool on = slide_on_screen(ss);
	enum obs_media_state media = obs_source_media_get_state(ss);
	printf("%s once+hide: after Next on the last slide: slide %lld, on screen %d, media state %d\n", kind, idx, on,
	       media);

	/* another Next keeps it ended */
	press_next(ss);
	bool on_again = slide_on_screen(ss);
	destroy_slideshow(ss);

	assert_int_equal(idx, SLIDES - 1);
	assert_false(on);
	assert_int_equal(media, OBS_MEDIA_STATE_ENDED);
	assert_false(on_again);
}

/* Control: "Loop" still wraps to the first slide. */
static void loop_next_on_last_slide_wraps(const char *kind)
{
	obs_source_t *ss = create_slideshow(kind, "loop", true);
	press_next(ss);
	press_next(ss);
	press_next(ss);
	long long idx = current_index(ss);
	bool on = slide_on_screen(ss);
	destroy_slideshow(ss);

	assert_int_equal(idx, 0);
	assert_true(on);
}

#define SLIDESHOW_TEST(name, kind)              \
	static void test_##name##_##kind(void **state) \
	{                                              \
		(void)state;                           \
		if (!obs_up)                           \
			skip();                        \
		name(#kind);                           \
	}

SLIDESHOW_TEST(once_next_on_last_slide_stays, slideshow_v2)
SLIDESHOW_TEST(once_hide_next_on_last_slide_ends, slideshow_v2)
SLIDESHOW_TEST(loop_next_on_last_slide_wraps, slideshow_v2)
SLIDESHOW_TEST(once_next_on_last_slide_stays, slideshow)
SLIDESHOW_TEST(once_hide_next_on_last_slide_ends, slideshow)
SLIDESHOW_TEST(loop_next_on_last_slide_wraps, slideshow)

static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return 0;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return 0;
	}

	register_types();
	obs_up = true;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_once_next_on_last_slide_stays_slideshow_v2),
		cmocka_unit_test(test_once_hide_next_on_last_slide_ends_slideshow_v2),
		cmocka_unit_test(test_loop_next_on_last_slide_wraps_slideshow_v2),
		cmocka_unit_test(test_once_next_on_last_slide_stays_slideshow),
		cmocka_unit_test(test_once_hide_next_on_last_slide_ends_slideshow),
		cmocka_unit_test(test_loop_next_on_last_slide_wraps_slideshow),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}

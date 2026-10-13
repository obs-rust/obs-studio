/* Regression test for obsproject/obs-studio#10852 (reported by
 * TheRealMakkus, reproduced by RytoEX): a cropped Media Source with Fade
 * show and hide transitions plays once. After it has played to its end,
 * hiding it and showing it again never plays it again, and it stays in the
 * audio mixer as if it were never closed.
 *
 * A scene item's show/hide transition only finishes when it is rendered:
 * obs_transition_video_render() notices the time is up and sends
 * transition_stop, which releases the item's source. A cropped item (or one
 * with a scale filter, a blend mode, ...) is drawn through a texture, and
 * render_item() skips such an item entirely while its source has no size.
 * A Media Source that has played to its end with "clear on media end" has
 * no size, so its hide transition never ran out: the source stayed active,
 * and showing the item again could not activate it anew, so "restart when
 * the source becomes active" never fired.
 *
 * The test builds a program scene with an item of a video source whose size
 * it controls, show/hide transitions on the item, and checks the source's
 * activation count and the transitions after hiding and showing:
 *
 *   - cropped item, source without size (the report): hiding must release
 *     the source, and showing it again must activate it again (once);
 *   - controls: the same item without crop, a cropped item of a source that
 *     has a size, and an item cropped to nothing release the source today.
 *
 * Item transitions run on the render thread, which needs core video, so the
 * test is built on Linux only and runs under xvfb-run; without a display the
 * cases skip. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <obs-internal.h>
#include <obs-scene.h>
#include <util/platform.h>

#define BOX_ID "test_item_transition_empty_box"
#define FADE_ID "test_item_transition_empty_fade"
#define TRANSITION_MS 200

static bool obs_up;

struct box {
	obs_source_t *source;
	volatile long size;
};

static const char *box_name(void *unused)
{
	(void)unused;
	return "box";
}

static void *box_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	struct box *b = bzalloc(sizeof(*b));
	b->source = source;
	b->size = 64;
	return b;
}

static void box_destroy(void *data)
{
	bfree(data);
}

static uint32_t box_size(void *data)
{
	struct box *b = data;
	return (uint32_t)os_atomic_load_long(&b->size);
}

static void box_render(void *data, gs_effect_t *effect)
{
	(void)data;
	(void)effect;
}

static const char *fade_name(void *unused)
{
	(void)unused;
	return "fade";
}

static void *fade_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void fade_destroy(void *data)
{
	(void)data;
}

static void fade_render(void *data, gs_effect_t *effect)
{
	(void)effect;
	obs_transition_video_render(data, NULL);
}

static float mix_a(void *data, float t)
{
	(void)data;
	return 1.0f - t;
}

static float mix_b(void *data, float t)
{
	(void)data;
	return t;
}

static bool fade_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
			      size_t channels, size_t sample_rate)
{
	return obs_transition_audio_render(data, ts_out, audio, mixers, channels, sample_rate, mix_a, mix_b);
}

static void register_sources(void)
{
	struct obs_source_info box = {
		.id = BOX_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = box_name,
		.create = box_create,
		.destroy = box_destroy,
		.get_width = box_size,
		.get_height = box_size,
		.video_render = box_render,
	};
	obs_register_source(&box);

	struct obs_source_info fade = {
		.id = FADE_ID,
		.type = OBS_SOURCE_TYPE_TRANSITION,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = fade_name,
		.create = fade_create,
		.destroy = fade_destroy,
		.video_render = fade_render,
		.audio_render = fade_audio_render,
	};
	obs_register_source(&fade);
}

struct fixture {
	obs_scene_t *program;
	obs_sceneitem_t *item;
	obs_source_t *box;
	obs_source_t *show;
	obs_source_t *hide;
};

static int fixture_setup(void **state)
{
	if (!obs_up) {
		*state = NULL;
		return 0;
	}

	struct fixture *fx = bzalloc(sizeof(*fx));
	*state = fx;
	fx->program = obs_scene_create("program");
	fx->box = obs_source_create(BOX_ID, "media", NULL, NULL);
	fx->show = obs_source_create_private(FADE_ID, "show fade", NULL);
	fx->hide = obs_source_create_private(FADE_ID, "hide fade", NULL);
	if (!fx->program || !fx->box || !fx->show || !fx->hide)
		return -1;

	fx->item = obs_scene_add(fx->program, fx->box);
	obs_sceneitem_set_transition(fx->item, true, fx->show);
	obs_sceneitem_set_transition(fx->item, false, fx->hide);
	obs_sceneitem_set_transition_duration(fx->item, true, TRANSITION_MS);
	obs_sceneitem_set_transition_duration(fx->item, false, TRANSITION_MS);
	obs_set_output_source(0, obs_scene_get_source(fx->program));
	os_sleep_ms(200);
	return 0;
}

static int fixture_teardown(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		return 0;

	obs_set_output_source(0, NULL);
	if (fx->program) {
		obs_source_remove(obs_scene_get_source(fx->program));
		obs_scene_release(fx->program);
	}
	obs_source_release(fx->show);
	obs_source_release(fx->hide);
	obs_source_release(fx->box);
	obs_wait_for_destroy_queue();
	bfree(fx);
	return 0;
}

static long active_refs(obs_source_t *source)
{
	return os_atomic_load_long(&source->activate_refs);
}

/* Wait for the item transition to run out (it runs on the render thread). */
static void wait_transitions(struct fixture *fx)
{
	os_sleep_ms(TRANSITION_MS);
	for (int i = 0; i < 100 && (obs_transition_is_active(fx->show) || obs_transition_is_active(fx->hide)); i++)
		os_sleep_ms(10);
	/* the transition_stop signal follows the flags */
	os_sleep_ms(100);
}

static void set_size(struct fixture *fx, long size)
{
	struct box *b = obs_obj_get_data(fx->box);
	os_atomic_set_long(&b->size, size);
	os_sleep_ms(100);
}

static void crop(struct fixture *fx, int each_side)
{
	struct obs_sceneitem_crop c = {each_side, each_side, each_side, each_side};
	obs_sceneitem_set_crop(fx->item, &c);
	os_sleep_ms(100);
}

/* Hide the item, then show it again, as the reporter did after the media
 * ended. */
static void hide_and_show(struct fixture *fx, const char *what)
{
	long before = active_refs(fx->box);
	if (before != 1) {
		fprintf(stderr, "%s: source activate_refs=%ld before hiding, expected 1\n", what, before);
		fail();
	}

	obs_sceneitem_set_visible(fx->item, false);
	if (!obs_transition_is_active(fx->hide)) {
		fprintf(stderr, "%s: the hide transition did not start\n", what);
		fail();
	}
	wait_transitions(fx);
	long hidden = active_refs(fx->box);
	bool hide_running = obs_transition_is_active(fx->hide);

	obs_sceneitem_set_visible(fx->item, true);
	wait_transitions(fx);
	long shown = active_refs(fx->box);
	bool show_running = obs_transition_is_active(fx->show);

	fprintf(stderr, "%s: activate_refs hidden=%ld (hide transition %s), shown again=%ld (show transition %s)\n",
		what, hidden, hide_running ? "still running" : "done", shown, show_running ? "still running" : "done");

	if (hidden != 0 || hide_running) {
		fprintf(stderr, "%s: hiding did not release the source\n", what);
		fail();
	}
	if (shown != 1 || show_running) {
		fprintf(stderr, "%s: showing again did not activate the source anew\n", what);
		fail();
	}
}

/* The report: a cropped Media Source that has played to its end (cleared,
 * so it has no size). */
static void cropped_item_of_empty_source(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();

	crop(fx, 4);
	set_size(fx, 0);
	hide_and_show(fx, "cropped, source without size");
}

/* Control: cropped to nothing, the source still has a size. */
static void item_cropped_to_nothing(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();

	crop(fx, 40);
	hide_and_show(fx, "cropped to nothing");
}

/* Control: no crop, the item is drawn directly. */
static void uncropped_item_of_empty_source(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();

	set_size(fx, 0);
	hide_and_show(fx, "not cropped, source without size");
}

/* Control: a cropped item of a source that has a size. */
static void cropped_item_of_sized_source(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();

	crop(fx, 4);
	hide_and_show(fx, "cropped, source with size");
}

static int group_setup(void **state)
{
	(void)state;
	/* obs_startup() and core video need a display on Linux (run under
	 * xvfb-run); the cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

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
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "skipped: obs_reset_audio failed\n");
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	register_sources();
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
		cmocka_unit_test_setup_teardown(cropped_item_of_empty_source, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(item_cropped_to_nothing, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(uncropped_item_of_empty_source, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(cropped_item_of_sized_source, fixture_setup, fixture_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

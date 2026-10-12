/* Regression test for upstream obsproject/obs-studio#4120 (reported by
 * Bobber-bob, confirmed by Fenrirthviti, diagnosed by PatTheMav): in Studio
 * Mode, hiding a source that is only in the Preview scene never makes it "not
 * visible", so "Shutdown source when not visible" (browser source, media
 * source) never fires.
 *
 * The frontend shows the Preview scene with obs_source_inc_showing(); only the
 * Program is on an output channel, so only the Program scene is active.
 * Hiding a scene item whose source has audio is not applied at once: it is
 * queued as an audio action for the audio thread, so the audio fades out on a
 * sample boundary. The audio thread only renders the active tree of the output
 * channels, so for a scene that is showing but not active the action is never
 * applied, and the source keeps its show reference.
 *
 * The cases count the show/hide and activate/deactivate callbacks of a source
 * with audio and of a video-only control, in a Preview-only scene, in a
 * Preview-only group and in the Program scene, for hiding, showing again and
 * removing the item. Applying the actions needs core video and audio, so the
 * test only builds on Linux and runs under xvfb-run; without a display the
 * cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <obs-internal.h>
#include <obs-scene.h>
#include <util/platform.h>

#define AV_ID "test_preview_hide_av"
#define VIDEO_ID "test_preview_hide_video"
/* a few video ticks at 30 fps */
#define WAIT_MS 500

static bool obs_up;

/* ------------------------------------------------------------------------ */
/* Source that counts its show/hide and activate/deactivate callbacks        */

struct counter {
	volatile long shown;
	volatile long hidden;
	volatile long activated;
	volatile long deactivated;
};

static const char *counter_name(void *unused)
{
	(void)unused;
	return "counter";
}

static void *counter_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(sizeof(struct counter));
}

static void counter_destroy(void *data)
{
	bfree(data);
}

static void counter_show(void *data)
{
	os_atomic_inc_long(&((struct counter *)data)->shown);
}

static void counter_hide(void *data)
{
	os_atomic_inc_long(&((struct counter *)data)->hidden);
}

static void counter_activate(void *data)
{
	os_atomic_inc_long(&((struct counter *)data)->activated);
}

static void counter_deactivate(void *data)
{
	os_atomic_inc_long(&((struct counter *)data)->deactivated);
}

static uint32_t counter_size(void *data)
{
	(void)data;
	return 16;
}

static void counter_render(void *data, gs_effect_t *effect)
{
	(void)data;
	(void)effect;
}

static void register_sources(void)
{
	struct obs_source_info info = {
		.id = AV_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_AUDIO,
		.get_name = counter_name,
		.create = counter_create,
		.destroy = counter_destroy,
		.show = counter_show,
		.hide = counter_hide,
		.activate = counter_activate,
		.deactivate = counter_deactivate,
		.get_width = counter_size,
		.get_height = counter_size,
		.video_render = counter_render,
	};
	obs_register_source(&info);

	info.id = VIDEO_ID;
	info.output_flags = OBS_SOURCE_VIDEO;
	obs_register_source(&info);
}

/* ------------------------------------------------------------------------ */
/* Fixture: Program scene on the output, Preview scene only showing          */

struct fixture {
	obs_scene_t *program;
	obs_scene_t *preview;
	obs_source_t *source;
	struct counter *count;
	bool preview_showing;
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
	fx->preview = obs_scene_create("preview");
	if (!fx->program || !fx->preview)
		return -1;
	obs_set_output_source(0, obs_scene_get_source(fx->program));
	return 0;
}

static int fixture_teardown(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		return 0;
	if (fx->preview_showing)
		obs_source_dec_showing(obs_scene_get_source(fx->preview));
	obs_set_output_source(0, NULL);
	/* the main canvas holds a reference to its scenes until they are removed */
	if (fx->program) {
		obs_source_remove(obs_scene_get_source(fx->program));
		obs_scene_release(fx->program);
	}
	if (fx->preview) {
		obs_source_remove(obs_scene_get_source(fx->preview));
		obs_scene_release(fx->preview);
	}
	obs_source_release(fx->source);
	obs_wait_for_destroy_queue();
	bfree(fx);
	return 0;
}

static void create_source(struct fixture *fx, const char *id)
{
	fx->source = obs_source_create(id, "counted", NULL, NULL);
	if (!fx->source) {
		fprintf(stderr, "could not create the %s source\n", id);
		fail();
	}
	fx->count = obs_obj_get_data(fx->source);
}

/* What the frontend does for the Preview in Studio Mode. */
static void show_preview(struct fixture *fx)
{
	obs_source_inc_showing(obs_scene_get_source(fx->preview));
	fx->preview_showing = true;
}

/* Wait a few video ticks for the show/hide callbacks to settle. */
static bool wait_counts(struct counter *c, long shown, long hidden)
{
	for (int i = 0; i < WAIT_MS / 10; i++) {
		if (os_atomic_load_long(&c->shown) == shown && os_atomic_load_long(&c->hidden) == hidden)
			return true;
		os_sleep_ms(10);
	}
	return false;
}

static void check_counts(struct fixture *fx, long shown, long hidden, long activated, long deactivated,
			 const char *when)
{
	struct counter *c = fx->count;
	bool settled = wait_counts(c, shown, hidden);
	/* give a wrong extra callback a chance to show up */
	os_sleep_ms(100);

	long s = os_atomic_load_long(&c->shown);
	long h = os_atomic_load_long(&c->hidden);
	long a = os_atomic_load_long(&c->activated);
	long d = os_atomic_load_long(&c->deactivated);
	bool showing = obs_source_showing(fx->source);
	bool want_showing = shown > hidden;

	if (!settled || s != shown || h != hidden || a != activated || d != deactivated || showing != want_showing) {
		fprintf(stderr,
			"%s: show=%ld hide=%ld activate=%ld deactivate=%ld showing=%d (show_refs=%ld), "
			"expected %ld/%ld/%ld/%ld showing=%d within %d ms\n",
			when, s, h, a, d, showing, os_atomic_load_long(&fx->source->show_refs), shown, hidden,
			activated, deactivated, want_showing, WAIT_MS);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                     */

/* Hide, show again and hide an item that is only in the Preview scene. */
static void toggle_preview_only(struct fixture *fx, const char *id)
{
	create_source(fx, id);
	obs_sceneitem_t *item = obs_scene_add(fx->preview, fx->source);
	show_preview(fx);
	check_counts(fx, 1, 0, 0, 0, "in the preview");

	obs_sceneitem_set_visible(item, false);
	check_counts(fx, 1, 1, 0, 0, "after hiding it in the preview");

	obs_sceneitem_set_visible(item, true);
	check_counts(fx, 2, 1, 0, 0, "after showing it again in the preview");

	obs_sceneitem_set_visible(item, false);
	check_counts(fx, 2, 2, 0, 0, "after hiding it again in the preview");

	obs_sceneitem_remove(item);
	check_counts(fx, 2, 2, 0, 0, "after removing the hidden item");
}

/* The steps from #4120, with a source that has audio (browser, media). */
static void hide_preview_only_audio(void **state)
{
	if (!obs_up)
		skip();
	toggle_preview_only(*state, AV_ID);
}

/* Control: a video-only source is hidden at once. */
static void hide_preview_only_video(void **state)
{
	if (!obs_up)
		skip();
	toggle_preview_only(*state, VIDEO_ID);
}

/* Hiding a group that is only in the Preview scene (groups carry audio). */
static void hide_preview_only_group(void **state)
{
	struct fixture *fx = *state;
	if (!obs_up)
		skip();
	create_source(fx, VIDEO_ID);
	obs_sceneitem_t *group = obs_scene_add_group2(fx->preview, "group", false);
	if (!group) {
		fprintf(stderr, "could not create the group\n");
		fail();
	}
	obs_scene_add(obs_sceneitem_group_get_scene(group), fx->source);
	show_preview(fx);
	check_counts(fx, 1, 0, 0, 0, "in a group in the preview");

	obs_sceneitem_set_visible(group, false);
	check_counts(fx, 1, 1, 0, 0, "after hiding the group in the preview");

	obs_sceneitem_set_visible(group, true);
	check_counts(fx, 2, 1, 0, 0, "after showing the group again in the preview");
}

/* Control: removing a visible item from the Preview scene. */
static void remove_preview_only_audio(void **state)
{
	struct fixture *fx = *state;
	if (!obs_up)
		skip();
	create_source(fx, AV_ID);
	obs_sceneitem_t *item = obs_scene_add(fx->preview, fx->source);
	show_preview(fx);
	check_counts(fx, 1, 0, 0, 0, "in the preview");

	obs_sceneitem_remove(item);
	check_counts(fx, 1, 1, 0, 0, "after removing it from the preview");
}

/* Control: in the Program scene the audio thread applies the hide. */
static void hide_program_audio(void **state)
{
	struct fixture *fx = *state;
	if (!obs_up)
		skip();
	create_source(fx, AV_ID);
	obs_sceneitem_t *item = obs_scene_add(fx->program, fx->source);
	check_counts(fx, 1, 0, 1, 0, "in the program");

	obs_sceneitem_set_visible(item, false);
	check_counts(fx, 1, 1, 1, 1, "after hiding it in the program");

	obs_sceneitem_set_visible(item, true);
	check_counts(fx, 2, 1, 2, 1, "after showing it again in the program");
}

/* A scene with a pending hide that moves from the Program to the Preview
 * (switching scenes in Studio Mode right after hiding something). */
static void hide_then_move_to_preview(void **state)
{
	struct fixture *fx = *state;
	if (!obs_up)
		skip();
	create_source(fx, AV_ID);
	obs_scene_t *other = obs_scene_create("other");
	obs_sceneitem_t *item = obs_scene_add(fx->preview, fx->source);
	obs_set_output_source(0, obs_scene_get_source(fx->preview));
	check_counts(fx, 1, 0, 1, 0, "in the program");

	show_preview(fx);
	obs_sceneitem_set_visible(item, false);
	obs_set_output_source(0, obs_scene_get_source(other));
	check_counts(fx, 1, 1, 1, 1, "after hiding it and moving its scene to the preview");

	obs_source_remove(obs_scene_get_source(other));
	obs_scene_release(other);
}

static int preview_setup(void **state)
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

static int preview_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(hide_preview_only_video, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(hide_program_audio, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(remove_preview_only_audio, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(hide_preview_only_audio, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(hide_preview_only_group, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(hide_then_move_to_preview, fixture_setup, fixture_teardown),
	};
	return cmocka_run_group_tests(tests, preview_setup, preview_teardown);
}

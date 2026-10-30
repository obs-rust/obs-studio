/* Regression test for upstream obsproject/obs-studio#5551 (reported by
 * keirakatana, reproduced by RytoEX with a VLC Source and a Media Source):
 * hiding a group does not stop a child whose scene item has show/hide
 * transitions. The child stays active, so a media source keeps playing and
 * the mixer keeps listing it.
 *
 * Hiding a group starts the hide transition of each visible child, and that
 * transition becomes an active child of the group's scene, holding the child
 * source. The child item itself stays visible, so when the group's scene is
 * deactivated its active tree does not list the hide transition, and the
 * transition (and the source in it) is never deactivated.
 *
 * The cases check the raw activate/show counts of the child source and of
 * both transitions, and measure the program mix (track 1) with a constant
 * tone. Controls: the same group without transitions, and hiding the child
 * item itself with its transitions. Hiding the group is applied by the audio
 * thread, which needs core video, so the test only builds on Linux and runs
 * under xvfb-run; without a display the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>

#include <obs-internal.h>
#include <obs-scene.h>
#include <util/platform.h>
#include <util/threading.h>

#define RATE 48000
#define LEVEL 0.5f
#define TONE_ID "test_group_hide_tone"
#define XFADE_ID "test_group_hide_xfade"
#define ITEM_TRANSITION_MS 300

static bool obs_up;

/* ------------------------------------------------------------------------ */
/* Constant-level audio source                                               */

struct tone {
	obs_source_t *source;
	pthread_t thread;
	os_event_t *stop;
};

static void *tone_thread(void *data)
{
	struct tone *t = data;
	static float buf[480];
	for (size_t i = 0; i < 480; i++)
		buf[i] = LEVEL;

	uint64_t next = os_gettime_ns();
	while (os_event_try(t->stop) == EAGAIN) {
		struct obs_source_audio a = {
			.data = {(uint8_t *)buf, (uint8_t *)buf},
			.frames = 480,
			.speakers = SPEAKERS_STEREO,
			.format = AUDIO_FORMAT_FLOAT_PLANAR,
			.samples_per_sec = RATE,
			.timestamp = next,
		};
		obs_source_output_audio(t->source, &a);
		next += 10000000ULL;
		os_sleepto_ns(next);
	}
	return NULL;
}

static const char *tone_name(void *unused)
{
	(void)unused;
	return "tone";
}

static void *tone_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	struct tone *t = bzalloc(sizeof(*t));
	t->source = source;
	os_event_init(&t->stop, OS_EVENT_TYPE_MANUAL);
	pthread_create(&t->thread, NULL, tone_thread, t);
	return t;
}

static void tone_destroy(void *data)
{
	struct tone *t = data;
	os_event_signal(t->stop);
	pthread_join(t->thread, NULL);
	os_event_destroy(t->stop);
	bfree(t);
}

/* ------------------------------------------------------------------------ */
/* Fade transition (what the frontend's Fade does to audio)                  */

static const char *xfade_name(void *unused)
{
	(void)unused;
	return "xfade";
}

static void *xfade_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void xfade_destroy(void *data)
{
	(void)data;
}

static void xfade_video_render(void *data, gs_effect_t *effect)
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

static bool xfade_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
			       size_t channels, size_t sample_rate)
{
	return obs_transition_audio_render(data, ts_out, audio, mixers, channels, sample_rate, mix_a, mix_b);
}

static void register_sources(void)
{
	struct obs_source_info tone = {
		.id = TONE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = tone_name,
		.create = tone_create,
		.destroy = tone_destroy,
	};
	obs_register_source(&tone);

	struct obs_source_info xfade = {
		.id = XFADE_ID,
		.type = OBS_SOURCE_TYPE_TRANSITION,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = xfade_name,
		.create = xfade_create,
		.destroy = xfade_destroy,
		.video_render = xfade_video_render,
		.audio_render = xfade_audio_render,
	};
	obs_register_source(&xfade);
}

/* ------------------------------------------------------------------------ */
/* Program level meter                                                       */

struct meter {
	pthread_mutex_t mutex;
	bool armed;
	uint64_t start_ts;
	uint64_t end_ts;
	uint64_t last_ts;
	double sum;
	uint64_t frames;
};

static void raw_audio(void *param, size_t mix_idx, struct audio_data *data)
{
	struct meter *m = param;
	(void)mix_idx;

	pthread_mutex_lock(&m->mutex);
	if (m->armed) {
		/* The mix lags real time by the audio buffering, so remember how
		 * far it has got: the measurement waits for the mix to reach the
		 * end of its window instead of trusting a fixed sleep. */
		if (data->timestamp > m->last_ts)
			m->last_ts = data->timestamp;
		if (data->timestamp >= m->start_ts && data->timestamp < m->end_ts) {
			const float *d = (const float *)data->data[0];
			for (uint32_t i = 0; i < data->frames; i++)
				m->sum += fabsf(d[i]);
			m->frames += data->frames;
		}
	}
	pthread_mutex_unlock(&m->mutex);
}

/* Average program level over the next 400 ms of the mix, as a fraction of the
 * tone. */
static double program_level(struct meter *m)
{
	const uint64_t now = os_gettime_ns();
	pthread_mutex_lock(&m->mutex);
	m->armed = true;
	m->start_ts = now;
	m->end_ts = now + 400000000ULL;
	m->last_ts = 0;
	m->sum = 0.0;
	m->frames = 0;
	pthread_mutex_unlock(&m->mutex);

	/* Wait for the mix to reach the end of the window on its own clock. A
	 * starved machine delays the measurement instead of emptying it: the
	 * 1000 ms sleep this replaces was already over when the audio thread
	 * had not run, and every case failed with no audio mixed. */
	const uint64_t deadline = now + 5000000000ULL;
	for (;;) {
		pthread_mutex_lock(&m->mutex);
		bool reached = m->last_ts >= m->end_ts;
		pthread_mutex_unlock(&m->mutex);
		if (reached || os_gettime_ns() >= deadline)
			break;
		os_sleep_ms(5);
	}

	pthread_mutex_lock(&m->mutex);
	m->armed = false;
	double sum = m->sum;
	uint64_t frames = m->frames;
	pthread_mutex_unlock(&m->mutex);

	if (!frames) {
		fprintf(stderr, "no program audio was mixed\n");
		fail();
	}
	return sum / (double)frames / LEVEL;
}

/* ------------------------------------------------------------------------ */
/* Fixture: program scene -> group -> tone                                   */

struct fixture {
	obs_scene_t *program;
	obs_scene_t *empty;
	obs_sceneitem_t *group;
	obs_sceneitem_t *item;
	obs_source_t *tone;
	obs_source_t *show;
	obs_source_t *hide;
	struct meter meter;
};

static int fixture_setup(void **state)
{
	if (!obs_up) {
		*state = NULL;
		return 0;
	}

	struct fixture *fx = bzalloc(sizeof(*fx));
	pthread_mutex_init(&fx->meter.mutex, NULL);
	fx->program = obs_scene_create("program");
	fx->empty = obs_scene_create("empty");
	fx->tone = obs_source_create(TONE_ID, "tone", NULL, NULL);
	fx->show = obs_source_create_private(XFADE_ID, "show fade", NULL);
	fx->hide = obs_source_create_private(XFADE_ID, "hide fade", NULL);
	*state = fx;
	if (!fx->program || !fx->empty || !fx->tone || !fx->show || !fx->hide)
		return -1;
	obs_add_raw_audio_callback(0, NULL, raw_audio, &fx->meter);
	return 0;
}

static int fixture_teardown(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		return 0;
	obs_remove_raw_audio_callback(0, raw_audio, &fx->meter);
	obs_set_output_source(0, NULL);
	/* the main canvas holds a reference to its scenes until they are removed */
	if (fx->program) {
		obs_source_remove(obs_scene_get_source(fx->program));
		obs_scene_release(fx->program);
	}
	if (fx->empty) {
		obs_source_remove(obs_scene_get_source(fx->empty));
		obs_scene_release(fx->empty);
	}
	obs_source_release(fx->show);
	obs_source_release(fx->hide);
	obs_source_release(fx->tone);
	obs_wait_for_destroy_queue();
	pthread_mutex_destroy(&fx->meter.mutex);
	bfree(fx);
	return 0;
}

static void check_source(obs_source_t *source, long want_active, long want_showing, const char *when)
{
	long active = os_atomic_load_long(&source->activate_refs);
	long showing = os_atomic_load_long(&source->show_refs);
	if (active != want_active || showing != want_showing) {
		fprintf(stderr, "%s: '%s' activate_refs=%ld show_refs=%ld, expected %ld/%ld\n", when,
			obs_source_get_name(source), active, showing, want_active, want_showing);
		fail();
	}
}

static void check_tone(struct fixture *fx, bool want_active, const char *when)
{
	check_source(fx->tone, want_active ? 1 : 0, want_active ? 1 : 0, when);
}

static void check_transitions_idle(struct fixture *fx, const char *when)
{
	check_source(fx->show, 0, 0, when);
	check_source(fx->hide, 0, 0, when);
}

static void check_level(struct fixture *fx, bool want_audible, const char *when)
{
	double level = program_level(&fx->meter);
	fprintf(stderr, "%s: program level %.2f of the tone\n", when, level);
	if (want_audible ? level < 0.9 : level > 0.01) {
		fprintf(stderr, "%s: program audio should be %s\n", when, want_audible ? "the full tone" : "silent");
		fail();
	}
}

/* Hiding an item whose source has audio is queued as an audio action; wait
 * for the audio thread to apply it. */
static void wait_inactive(obs_sceneitem_t *item, const char *what)
{
	for (int i = 0; i < 200 && os_atomic_load_long(&item->active_refs) > 0; i++)
		os_sleep_ms(10);
	if (os_atomic_load_long(&item->active_refs) > 0) {
		fprintf(stderr, "%s is still active 2 s after hiding it\n", what);
		fail();
	}
}

/* An item transition runs on the render threads; wait until it has ended. */
static void wait_transition_done(obs_source_t *transition)
{
	os_sleep_ms(ITEM_TRANSITION_MS);
	for (int i = 0; i < 200 && obs_transition_is_active(transition); i++)
		os_sleep_ms(10);
	/* the transition_stop signal follows the flags */
	os_sleep_ms(100);
}

static void build_scene(struct fixture *fx, bool transitions)
{
	fx->group = obs_scene_add_group2(fx->program, "group", false);
	if (!fx->group) {
		fprintf(stderr, "could not create the group\n");
		fail();
	}
	fx->item = obs_scene_add(obs_sceneitem_group_get_scene(fx->group), fx->tone);
	if (transitions) {
		obs_sceneitem_set_transition(fx->item, true, fx->show);
		obs_sceneitem_set_transition(fx->item, false, fx->hide);
		obs_sceneitem_set_transition_duration(fx->item, true, ITEM_TRANSITION_MS);
		obs_sceneitem_set_transition_duration(fx->item, false, ITEM_TRANSITION_MS);
	}
	obs_set_output_source(0, obs_scene_get_source(fx->program));
	os_sleep_ms(300);
	check_tone(fx, true, "with the group on the program");
	check_level(fx, true, "with the group on the program");
}

static void hide_group(struct fixture *fx)
{
	obs_sceneitem_set_visible(fx->group, false);
	wait_inactive(fx->group, "the group");
	os_sleep_ms(ITEM_TRANSITION_MS + 200);
}

static void switch_away(struct fixture *fx)
{
	obs_set_output_source(0, obs_scene_get_source(fx->empty));
	check_tone(fx, false, "after switching to an empty scene");
	check_transitions_idle(fx, "after switching to an empty scene");
}

/* The steps from #5551: show and hide transitions on a source in a group,
 * then hide the group. Showing the group again must not stack a second
 * activation either. */
static void hide_group_with_child_transitions(void **state)
{
	struct fixture *fx = *state;
	if (!obs_up)
		skip();
	build_scene(fx, true);

	hide_group(fx);
	check_level(fx, false, "after hiding the group");
	check_tone(fx, false, "after hiding the group");
	check_transitions_idle(fx, "after hiding the group");

	obs_sceneitem_set_visible(fx->group, true);
	wait_transition_done(fx->show);
	check_tone(fx, true, "after showing the group again");
	check_transitions_idle(fx, "after showing the group again");
	check_level(fx, true, "after showing the group again");

	switch_away(fx);
}

/* Hiding the group while the child's show transition is still running. */
static void hide_group_during_child_show_transition(void **state)
{
	struct fixture *fx = *state;
	if (!obs_up)
		skip();
	build_scene(fx, true);
	obs_sceneitem_set_transition_duration(fx->item, true, 5000);

	hide_group(fx);
	obs_sceneitem_set_visible(fx->group, true);
	os_sleep_ms(200);
	if (!obs_transition_is_active(fx->show)) {
		fprintf(stderr, "the child's show transition is not running\n");
		fail();
	}
	hide_group(fx);
	check_tone(fx, false, "after hiding the group during the show transition");
	check_transitions_idle(fx, "after hiding the group during the show transition");

	obs_sceneitem_set_transition_duration(fx->item, true, ITEM_TRANSITION_MS);
	obs_sceneitem_set_visible(fx->group, true);
	wait_transition_done(fx->show);
	check_tone(fx, true, "after showing the group again");
	check_transitions_idle(fx, "after showing the group again");

	switch_away(fx);
}

/* Control: without item transitions, hiding the group already works. */
static void hide_group_without_transitions(void **state)
{
	struct fixture *fx = *state;
	if (!obs_up)
		skip();
	build_scene(fx, false);

	hide_group(fx);
	check_tone(fx, false, "after hiding the group");
	check_level(fx, false, "after hiding the group");

	obs_sceneitem_set_visible(fx->group, true);
	check_tone(fx, true, "after showing the group again");
	check_level(fx, true, "after showing the group again");

	switch_away(fx);
}

/* Control: hiding the child item itself plays its hide transition and then
 * deactivates it. */
static void hide_child_with_transitions(void **state)
{
	struct fixture *fx = *state;
	if (!obs_up)
		skip();
	build_scene(fx, true);

	obs_sceneitem_set_visible(fx->item, false);
	wait_transition_done(fx->hide);
	wait_inactive(fx->item, "the child");
	check_tone(fx, false, "after hiding the child");
	check_transitions_idle(fx, "after hiding the child");
	check_level(fx, false, "after hiding the child");

	obs_sceneitem_set_visible(fx->item, true);
	wait_transition_done(fx->show);
	check_tone(fx, true, "after showing the child again");
	check_transitions_idle(fx, "after showing the child again");
	check_level(fx, true, "after showing the child again");

	switch_away(fx);
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

	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
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
		cmocka_unit_test_setup_teardown(hide_group_without_transitions, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(hide_child_with_transitions, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(hide_group_with_child_transitions, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(hide_group_during_child_show_transition, fixture_setup,
						fixture_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

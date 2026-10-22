/* Characterization of the scene item visibility audio ramp in
 * libobs/obs-scene.c (apply_scene_item_audio_actions,
 * apply_scene_item_volume and process_all_audio_actions) before its Rust
 * port, through the real libobs.
 *
 * Showing or hiding a scene item whose source has audio does not change the
 * mix at once: obs_sceneitem_set_visible() queues an audio action with the
 * time of the call, and the audio thread applies it on the sample of the
 * scene's audio tick that the time falls on, through a 0/1 gain buffer.
 * Each hide it applies drops one of the item's active references.
 *
 * The cases capture the program mix (track 1) with obs_add_raw_audio_callback
 * while constant-level tones play, and pin the sample of each change:
 * - a hide of the only audible item, and of one item over a second one;
 * - a show over a second item;
 * - three toggles queued back to back: all three are applied, and the two
 *   hides drop the two references the item had (one from its add, one from
 *   the show), so its source is deactivated once.
 *
 * The sample index does not depend on how fast the machine is. The action
 * time is taken inside obs_sceneitem_set_visible(), so the test brackets the
 * call with os_gettime_ns() and accepts exactly the samples that a time in
 * that bracket maps to: floor((t - tick) * rate / 1e9), the formula of the
 * C. The waits only bound how long the test waits for the audio thread;
 * they are not part of any check.
 *
 * That holds only if every tone of the scene has its audio in each tick
 * the audio thread renders while the action waits. On a loaded machine a
 * tone's thread can wake too late for a tick: libobs then marks the tone's
 * audio pending and leaves it out of that tick. When no visible item of the
 * scene has audio in a tick, the scene has none either (its own audio is
 * pending), and libobs applies every queued action at once, so a hide shows
 * up on sample 0 (characterized, not endorsed: process_all_audio_actions()).
 * Neither says where the action would have landed, so the capture also
 * records, from libobs, which tones and whether the scene were left out of
 * each tick, and a toggle whose capture has such a tick is undone and tried
 * again. The pending flags come from the sources' buffers and the scene's
 * render, not from the code under test.
 *
 * The audio thread renders a tick some time after the tick's time: one tick
 * plus the audio buffering, which libobs adds when a source's audio comes
 * late and which can reach most of a second. So the capture takes every
 * tick rendered after it starts, however old its time, or a tick that
 * applied the action at once could fall before the captured ones. And
 * libobs renders without delivering the ticks it adds as buffering, which
 * the raw callback can't see, so the test sets fixed buffering: libobs adds
 * all of it on the first tick, before any case, and never again.
 *
 * Public API, plus the item's active_refs and the source's activate_refs
 * (obs-scene.h, obs-internal.h), as test_group_hide_transition_audio reads
 * them; the structs stay C in the hybrid libobs, so the test covers it too.
 * Applying the actions needs core video and audio, so the test only builds
 * on Linux and runs under xvfb-run; without a display the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs-internal.h>
#include <obs-scene.h>
#include <util/platform.h>
#include <util/threading.h>

#define RATE 48000
#define FRAMES AUDIO_OUTPUT_FRAMES
#define LEVEL_A 0.5f
#define LEVEL_B 0.25f
#define TONE_ID "test_scene_audio_actions_tone"
#define MAX_TICKS 512
/* how long to wait for the audio thread before failing (not a check) */
#define DEADLINE_NS 20000000000ULL

static bool obs_up;

/* ------------------------------------------------------------------------ */
/* Constant-level audio source: "level" in its settings                      */

struct tone {
	obs_source_t *source;
	float level;
	pthread_t thread;
	os_event_t *stop;
};

static void *tone_thread(void *data)
{
	struct tone *t = data;
	float buf[480];
	for (size_t i = 0; i < 480; i++)
		buf[i] = t->level;

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
	struct tone *t = bzalloc(sizeof(*t));
	t->source = source;
	t->level = (float)obs_data_get_double(settings, "level");
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

static obs_source_t *create_tone(const char *name, float level)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_double(settings, "level", level);
	obs_source_t *source = obs_source_create(TONE_ID, name, settings, NULL);
	obs_data_release(settings);
	return source;
}

/* ------------------------------------------------------------------------ */
/* Program mix capture: channel 0 of track 1, one entry per audio tick       */

struct capture {
	pthread_mutex_t mutex;
	bool on;
	size_t num;
	uint64_t ts[MAX_TICKS];
	float data[MAX_TICKS][FRAMES];
	/* the tones of the scene, and which of them libobs left out of each
	 * tick because their audio was not there yet (pending) */
	obs_source_t *tones[2];
	bool left_out[MAX_TICKS][2];
	/* the scene, and whether it had no audio in each tick (pending) */
	obs_source_t *scene;
	bool scene_left_out[MAX_TICKS];
	/* the item being toggled, whether obs_sceneitem_set_visible() has
	 * returned, and whether its action had been applied by the end of
	 * each tick (its queue empty after the call) */
	obs_sceneitem_t *item;
	bool called;
	bool applied[MAX_TICKS];
};

static void raw_audio(void *param, size_t mix_idx, struct audio_data *data)
{
	struct capture *c = param;
	(void)mix_idx;

	pthread_mutex_lock(&c->mutex);
	if (c->on && c->num < MAX_TICKS && data->frames == FRAMES) {
		c->ts[c->num] = data->timestamp;
		memcpy(c->data[c->num], data->data[0], sizeof(float) * FRAMES);
		/* raw audio callbacks run on the audio thread right after it
		 * rendered this tick, so audio_pending is this tick's */
		for (size_t i = 0; i < 2; i++)
			c->left_out[c->num][i] = c->tones[i] && c->tones[i]->audio_pending;
		c->scene_left_out[c->num] = c->scene && c->scene->audio_pending;
		bool applied = false;
		if (c->item && c->called) {
			pthread_mutex_lock(&c->item->actions_mutex);
			applied = c->item->audio_actions.num == 0;
			pthread_mutex_unlock(&c->item->actions_mutex);
		}
		c->applied[c->num] = applied;
		c->num++;
	}
	pthread_mutex_unlock(&c->mutex);
}

/* Capture every tick the audio thread renders from now on. */
static void capture_start(struct capture *c, obs_sceneitem_t *item)
{
	pthread_mutex_lock(&c->mutex);
	c->on = true;
	c->num = 0;
	c->item = item;
	c->called = false;
	pthread_mutex_unlock(&c->mutex);
}

static void capture_called(struct capture *c)
{
	pthread_mutex_lock(&c->mutex);
	c->called = true;
	pthread_mutex_unlock(&c->mutex);
}

/* Wait until a tick at or after ts has been captured, then stop. */
static void capture_until(struct capture *c, uint64_t ts)
{
	uint64_t deadline = os_gettime_ns() + DEADLINE_NS;
	for (;;) {
		pthread_mutex_lock(&c->mutex);
		bool done = c->num && (c->ts[c->num - 1] >= ts || c->num == MAX_TICKS);
		if (done)
			c->on = false;
		pthread_mutex_unlock(&c->mutex);
		if (done)
			return;
		if (os_gettime_ns() > deadline) {
			fprintf(stderr, "the mix did not reach the tick at %llu\n", (unsigned long long)ts);
			fail();
		}
		os_sleep_ms(10);
	}
}

static uint64_t tick_ns(void)
{
	return util_mul_div64(FRAMES, 1000000000ULL, RATE);
}

/* Wait until a whole captured tick has exactly this level. */
static void wait_level(struct capture *c, float level, const char *when)
{
	uint64_t deadline = os_gettime_ns() + DEADLINE_NS;
	for (;;) {
		capture_start(c, NULL);
		os_sleep_ms(50);
		pthread_mutex_lock(&c->mutex);
		c->on = false;
		bool found = false;
		for (size_t t = 0; t < c->num && !found; t++) {
			size_t i = 0;
			while (i < FRAMES && c->data[t][i] == level)
				i++;
			found = i == FRAMES;
		}
		pthread_mutex_unlock(&c->mutex);
		if (found)
			return;
		if (os_gettime_ns() > deadline) {
			fprintf(stderr, "%s: the mix never settled at %.2f\n", when, level);
			fail();
		}
	}
}

/* True when libobs left a tone of the scene, or the scene, out of a
 * captured tick. A scene whose items are all hidden has no audio either,
 * so after the action is applied that counts only if `after` is not
 * silence. */
static bool audio_left_out(struct capture *c, float after, const char *what)
{
	for (size_t t = 0; t < c->num; t++) {
		for (size_t i = 0; i < 2; i++) {
			if (!c->left_out[t][i])
				continue;
			fprintf(stderr, "%s: tick %zu (ts %llu) has no audio from %s (it came late), trying again\n",
				what, t, (unsigned long long)c->ts[t], obs_source_get_name(c->tones[i]));
			return true;
		}
		if (c->scene_left_out[t] && (after != 0.0f || !t || !c->applied[t - 1])) {
			fprintf(stderr, "%s: tick %zu (ts %llu) has no audio from the scene, trying again\n", what, t,
				(unsigned long long)c->ts[t]);
			return true;
		}
	}
	return false;
}

/* The samples a time t in [t0, t1] maps to within the tick at ts, as the C
 * places an action: floor((t - ts) * rate / 1e9), the times before the tick
 * at 0. False when no time in the bracket falls on this tick. */
static bool frame_range(uint64_t ts, uint64_t t0, uint64_t t1, uint64_t *lo, uint64_t *hi)
{
	uint64_t end = ts + tick_ns(); /* first time of the next tick */
	if (t1 < ts || t0 >= end)
		return false;
	uint64_t a = t0 < ts ? ts : t0;
	uint64_t b = t1 >= end ? end - 1 : t1;
	*lo = util_mul_div64(a - ts, RATE, 1000000000ULL);
	*hi = util_mul_div64(b - ts, RATE, 1000000000ULL);
	return true;
}

/* The capture from before the change to well after it must read `before`
 * up to one sample and `after` from it on, the ticks contiguous, and that
 * sample must be one the bracket [t0, t1] maps to. */
static void check_change(struct capture *c, uint64_t t0, uint64_t t1, float before, float after, const char *what)
{
	size_t change_tick = SIZE_MAX;
	size_t change_frame = 0;

	if (c->num < 3) {
		fprintf(stderr, "%s: only %zu ticks captured\n", what, c->num);
		fail();
	}
	for (size_t t = 0; t < c->num; t++) {
		if (t && c->ts[t] - c->ts[t - 1] > tick_ns() + 1) {
			fprintf(stderr, "%s: ticks %zu and %zu are not contiguous (%llu ns apart)\n", what, t - 1, t,
				(unsigned long long)(c->ts[t] - c->ts[t - 1]));
			fail();
		}
		for (size_t i = 0; i < FRAMES; i++) {
			float want = change_tick == SIZE_MAX ? before : after;
			float got = c->data[t][i];
			if (got == want)
				continue;
			if (change_tick == SIZE_MAX && got == after) {
				change_tick = t;
				change_frame = i;
				continue;
			}
			fprintf(stderr, "%s: tick %zu (ts %llu) sample %zu is %f, expected %f\n", what, t,
				(unsigned long long)c->ts[t], i, got, want);
			fail();
		}
	}
	if (change_tick == SIZE_MAX) {
		fprintf(stderr, "%s: the mix never changed from %.2f\n", what, before);
		fail();
	}

	uint64_t ts = c->ts[change_tick];
	uint64_t lo = 0, hi = 0;
	bool in_tick = frame_range(ts, t0, t1, &lo, &hi);
	fprintf(stderr,
		"%s: changed at sample %zu of the tick at %+lld ns from the call (%llu ns long), allowed %s%llu-%llu\n",
		what, change_frame, (long long)(ts - t0), (unsigned long long)(t1 - t0), in_tick ? "" : "none, ",
		(unsigned long long)lo, (unsigned long long)hi);
	if (!in_tick || change_frame < lo || change_frame > hi) {
		fprintf(stderr, "%s: the change is not on the sample of the call\n", what);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* Fixture: a program scene with tone A, and tone B in some cases            */

struct fixture {
	obs_scene_t *program;
	obs_scene_t *empty;
	obs_source_t *a;
	obs_source_t *b;
	obs_sceneitem_t *item_a;
	obs_sceneitem_t *item_b;
	struct capture *capture;
};

static int fixture_setup(void **state)
{
	if (!obs_up) {
		*state = NULL;
		return 0;
	}

	struct fixture *fx = bzalloc(sizeof(*fx));
	fx->capture = bzalloc(sizeof(*fx->capture));
	pthread_mutex_init(&fx->capture->mutex, NULL);
	fx->program = obs_scene_create("program");
	fx->empty = obs_scene_create("empty");
	fx->a = create_tone("tone a", LEVEL_A);
	fx->b = create_tone("tone b", LEVEL_B);
	*state = fx;
	if (!fx->program || !fx->empty || !fx->a || !fx->b)
		return -1;
	obs_add_raw_audio_callback(0, NULL, raw_audio, fx->capture);
	return 0;
}

static int fixture_teardown(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		return 0;
	obs_remove_raw_audio_callback(0, raw_audio, fx->capture);
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
	obs_source_release(fx->a);
	obs_source_release(fx->b);
	obs_wait_for_destroy_queue();
	pthread_mutex_destroy(&fx->capture->mutex);
	bfree(fx->capture);
	bfree(fx);
	return 0;
}

/* Wait until the audio thread has applied every queued action of the item. */
static void wait_applied(obs_sceneitem_t *item, const char *what)
{
	uint64_t deadline = os_gettime_ns() + DEADLINE_NS;
	for (;;) {
		pthread_mutex_lock(&item->actions_mutex);
		size_t pending = item->audio_actions.num;
		pthread_mutex_unlock(&item->actions_mutex);
		if (!pending)
			return;
		if (os_gettime_ns() > deadline) {
			fprintf(stderr, "%s: %zu audio actions were never applied\n", what, pending);
			fail();
		}
		os_sleep_ms(10);
	}
}

static void check_refs(obs_sceneitem_t *item, long want_item, obs_source_t *source, long want_active, const char *when)
{
	long item_refs = os_atomic_load_long(&item->active_refs);
	long active = os_atomic_load_long(&source->activate_refs);
	if (item_refs != want_item || active != want_active) {
		fprintf(stderr, "%s: item active_refs=%ld, source activate_refs=%ld, expected %ld/%ld\n", when,
			item_refs, active, want_item, want_active);
		fail();
	}
}

static void play(struct fixture *fx, bool with_b)
{
	fx->item_a = obs_scene_add(fx->program, fx->a);
	if (with_b)
		fx->item_b = obs_scene_add(fx->program, fx->b);
	pthread_mutex_lock(&fx->capture->mutex);
	fx->capture->tones[0] = fx->a;
	fx->capture->tones[1] = with_b ? fx->b : NULL;
	fx->capture->scene = obs_scene_get_source(fx->program);
	pthread_mutex_unlock(&fx->capture->mutex);
	obs_set_output_source(0, obs_scene_get_source(fx->program));
	wait_applied(fx->item_a, "adding tone a");
	if (with_b)
		wait_applied(fx->item_b, "adding tone b");
	wait_level(fx->capture, with_b ? LEVEL_A + LEVEL_B : LEVEL_A, "playing");
}

/* Toggle the item and capture the mix of every tick rendered from just
 * before the call until three ticks after it. A show of a hidden item takes
 * its active reference at once, in obs_sceneitem_set_visible(), not when the
 * action is applied. When libobs left a tone or the scene out of a captured
 * tick, the capture can't show where the action landed: toggle the item
 * back, wait until the mix is back at `before`, and toggle it again. */
static void toggle(struct fixture *fx, obs_sceneitem_t *item, bool visible, float before, float after, uint64_t *t0,
		   uint64_t *t1)
{
	const char *what = visible ? "showing" : "hiding";
	uint64_t deadline = os_gettime_ns() + DEADLINE_NS;
	for (;;) {
		capture_start(fx->capture, item);
		*t0 = os_gettime_ns();
		obs_sceneitem_set_visible(item, visible);
		*t1 = os_gettime_ns();
		capture_called(fx->capture);
		if (visible)
			check_refs(item, 1, item->source, 1, "right after showing");
		wait_applied(item, what);
		capture_until(fx->capture, *t1 + 3 * tick_ns());
		if (!audio_left_out(fx->capture, after, what))
			return;
		if (os_gettime_ns() > deadline) {
			fprintf(stderr, "%s: a tone's or the scene's audio was left out in every try\n", what);
			fail();
		}
		obs_sceneitem_set_visible(item, !visible);
		wait_applied(item, "toggling back");
		wait_level(fx->capture, before, "toggling back");
	}
}

static void switch_away(struct fixture *fx)
{
	obs_set_output_source(0, obs_scene_get_source(fx->empty));
}

/* The only audible item is hidden: the mix goes silent on the sample of the
 * call, and the item's only active reference is dropped. */
static void hide_only_item(void **state)
{
	struct fixture *fx = *state;
	uint64_t t0, t1;
	if (!obs_up)
		skip();
	play(fx, false);
	check_refs(fx->item_a, 1, fx->a, 1, "playing");

	toggle(fx, fx->item_a, false, LEVEL_A, 0.0f, &t0, &t1);
	check_change(fx->capture, t0, t1, LEVEL_A, 0.0f, "hiding the only item");
	check_refs(fx->item_a, 0, fx->a, 0, "after hiding the only item");

	switch_away(fx);
}

/* One of two audible items is hidden: only its part of the mix stops, on
 * the sample of the call. */
static void hide_over_other_item(void **state)
{
	struct fixture *fx = *state;
	uint64_t t0, t1;
	if (!obs_up)
		skip();
	play(fx, true);

	toggle(fx, fx->item_a, false, LEVEL_A + LEVEL_B, LEVEL_B, &t0, &t1);
	check_change(fx->capture, t0, t1, LEVEL_A + LEVEL_B, LEVEL_B, "hiding one of two items");
	check_refs(fx->item_a, 0, fx->a, 0, "after hiding one of two items");
	check_refs(fx->item_b, 1, fx->b, 1, "the other item after hiding one of two");

	switch_away(fx);
}

/* A hidden item is shown while another one plays: its part of the mix
 * starts on the sample of the call. The show takes its active reference at
 * once, in obs_sceneitem_set_visible(), not when the action is applied. */
static void show_over_other_item(void **state)
{
	struct fixture *fx = *state;
	uint64_t t0, t1;
	if (!obs_up)
		skip();
	play(fx, true);
	obs_sceneitem_set_visible(fx->item_a, false);
	wait_applied(fx->item_a, "hiding tone a");
	wait_level(fx->capture, LEVEL_B, "after hiding tone a");

	toggle(fx, fx->item_a, true, LEVEL_B, LEVEL_A + LEVEL_B, &t0, &t1);
	check_change(fx->capture, t0, t1, LEVEL_B, LEVEL_A + LEVEL_B, "showing over another item");
	check_refs(fx->item_a, 1, fx->a, 1, "after showing");

	switch_away(fx);
}

/* Hide, show and hide again, back to back. All three actions are applied
 * (the item ends hidden and the mix silent), and both hides are counted:
 * the item's references (1 from the add, +1 from the show) drop to 0, and
 * the source is deactivated. */
static void toggles_back_to_back(void **state)
{
	struct fixture *fx = *state;
	if (!obs_up)
		skip();
	play(fx, false);

	obs_sceneitem_set_visible(fx->item_a, false);
	obs_sceneitem_set_visible(fx->item_a, true);
	check_refs(fx->item_a, 2, fx->a, 1, "after hide and show");
	obs_sceneitem_set_visible(fx->item_a, false);
	wait_applied(fx->item_a, "hide, show, hide");
	check_refs(fx->item_a, 0, fx->a, 0, "after hide, show, hide");
	wait_level(fx->capture, 0.0f, "after hide, show, hide");
	if (obs_sceneitem_visible(fx->item_a)) {
		fprintf(stderr, "the item is visible after hide, show, hide\n");
		fail();
	}

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

	/* Fixed buffering: libobs adds it all on the first tick (see the top).
	 * About 200 ms leaves a tone's thread that much time to be late before its
	 * audio is left out, and keeps the waits short. */
	struct obs_audio_info2 oai = {
		.samples_per_sec = RATE,
		.speakers = SPEAKERS_STEREO,
		.max_buffering_ms = 200,
		.fixed_buffering = true,
	};
	if (!obs_reset_audio2(&oai)) {
		fprintf(stderr, "skipped: obs_reset_audio2 failed\n");
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	struct obs_source_info tone = {
		.id = TONE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = tone_name,
		.create = tone_create,
		.destroy = tone_destroy,
	};
	obs_register_source(&tone);
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
		cmocka_unit_test_setup_teardown(hide_only_item, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(hide_over_other_item, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(show_over_other_item, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(toggles_back_to_back, fixture_setup, fixture_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

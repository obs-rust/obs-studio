#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-internal.h"
#include "obs-scene.h"

/*
 * Characterization of the scene item visibility audio ramp in
 * libobs/obs-scene-audio-actions.c, before its Rust port:
 * - apply_scene_item_audio_actions() consumes the item's queued
 *   {visible, timestamp} actions up to the end of the audio tick at ts,
 *   places each one on the sample floor((timestamp - ts) * rate / 1e9) (a
 *   timestamp before the tick on sample 0), fills the 0/1 gain buffer, and
 *   drops one active reference per hide (obs_source_remove_active_child()
 *   when the count reaches 0);
 * - apply_scene_item_volume() applies them only when the first queued action
 *   falls before ts + the tick's duration (or ts is 0);
 * - process_all_audio_actions() applies everything, whatever the time.
 * They pin the current C behavior; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, with a fake scene item: the real struct,
 * zeroed, with its actions, visibility, active_refs and mutex set. Its scene
 * has no source, so obs_source_remove_active_child() (real libobs) only logs
 * that its parent is NULL; the test counts those log lines to count the
 * removals. test_scene_audio_actions covers the callers through the real
 * libobs, and test_preview_only_hide the preview-only caller.
 */

#define FRAMES AUDIO_OUTPUT_FRAMES
#define NS 1000000000ULL
/* a tick timestamp well away from 0, which the C treats as "no tick" */
#define TICK 1000000000000ULL
/* neither 0 nor 1: a sample the C did not write */
#define UNWRITTEN 0.5f

/* ------------------------------------------------------------------------ */
/* Removals: obs_source_remove_active_child() logs its NULL parent           */

static long removals;

static void log_handler(int level, const char *format, va_list args, void *param)
{
	char msg[256];
	(void)level;
	(void)param;
	vsnprintf(msg, sizeof(msg), format, args);
	if (strstr(msg, "obs_source_remove_active_child"))
		removals++;
}

/* ------------------------------------------------------------------------ */
/* Fake scene item                                                           */

struct fake {
	struct obs_scene scene;
	struct obs_scene_item item;
	float buf[FRAMES + 1];
};

static struct fake *fake_new(bool visible, long refs)
{
	struct fake *f = bzalloc(sizeof(*f));
	f->item.parent = &f->scene;
	f->item.visible = visible;
	f->item.active_refs = refs;
	pthread_mutex_init(&f->item.actions_mutex, NULL);
	for (size_t i = 0; i <= FRAMES; i++)
		f->buf[i] = UNWRITTEN;
	removals = 0;
	return f;
}

static void fake_free(struct fake *f)
{
	/* the C never writes past the tick */
	assert_true(f->buf[FRAMES] == UNWRITTEN);
	da_free(f->item.audio_actions);
	pthread_mutex_destroy(&f->item.actions_mutex);
	bfree(f);
}

static void push(struct fake *f, bool visible, uint64_t timestamp)
{
	struct item_action action = {.visible = visible, .timestamp = timestamp};
	da_push_back(f->item.audio_actions, &action);
}

/* The first time that the C places on frame k of the tick at ts. */
static uint64_t at_frame(uint64_t ts, uint64_t k, uint64_t rate)
{
	return ts + (k * NS + rate - 1) / rate;
}

/* buf[from, to) is all `value` */
static void check_span(const struct fake *f, size_t from, size_t to, float value)
{
	for (size_t i = from; i < to; i++) {
		if (f->buf[i] != value) {
			fprintf(stderr, "sample %zu is %f, expected %f (span %zu-%zu)\n", i, f->buf[i], value, from,
				to);
			fail();
		}
	}
}

static void check_untouched(const struct fake *f)
{
	check_span(f, 0, FRAMES + 1, UNWRITTEN);
}

static void check_item(const struct fake *f, bool visible, long refs, long removed, size_t pending)
{
	assert_int_equal(f->item.visible, visible);
	assert_int_equal(f->item.active_refs, refs);
	assert_int_equal(removals, removed);
	assert_int_equal(f->item.audio_actions.num, pending);
}

/* ------------------------------------------------------------------------ */
/* apply_scene_item_audio_actions                                            */

/* No actions: the buffer is the current visibility, whole. */
static void no_actions(void **state)
{
	(void)state;
	struct fake *f = fake_new(true, 1);
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, FRAMES, 1.0f);
	check_item(f, true, 1, 0, 0);
	fake_free(f);

	f = fake_new(false, 1);
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, FRAMES, 0.0f);
	check_item(f, false, 1, 0, 0);
	fake_free(f);

	/* no buffer: nothing to do */
	f = fake_new(true, 1);
	apply_scene_item_audio_actions(&f->item, NULL, TICK, 48000);
	check_item(f, true, 1, 0, 0);
	fake_free(f);
}

/* A hide, and a show, on every frame of the tick at 48 kHz: the change is
 * on exactly that frame, and one nanosecond earlier is one frame earlier. */
static void one_change_on_each_frame(void **state)
{
	(void)state;
	for (uint64_t k = 0; k < FRAMES; k++) {
		struct fake *f = fake_new(true, 1);
		push(f, false, at_frame(TICK, k, 48000));
		apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
		check_span(f, 0, k, 1.0f);
		check_span(f, k, FRAMES, 0.0f);
		check_item(f, false, 0, 1, 0);
		fake_free(f);

		f = fake_new(false, 1);
		push(f, true, at_frame(TICK, k, 48000));
		apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
		check_span(f, 0, k, 0.0f);
		check_span(f, k, FRAMES, 1.0f);
		check_item(f, true, 1, 0, 0);
		fake_free(f);

		if (k) {
			f = fake_new(true, 1);
			push(f, false, at_frame(TICK, k, 48000) - 1);
			apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
			check_span(f, 0, k - 1, 1.0f);
			check_span(f, k - 1, FRAMES, 0.0f);
			fake_free(f);
		}
	}
}

/* The same placement at the rates libobs offers, and odd ones. */
static void one_change_at_each_rate(void **state)
{
	(void)state;
	static const uint64_t rates[] = {8000, 11025, 16000, 22050, 24000, 32000, 44100, 48000, 88200, 96000, 192000};
	static const uint64_t frames[] = {0, 1, 2, 511, 512, 1000, 1022, 1023};

	for (size_t r = 0; r < sizeof(rates) / sizeof(rates[0]); r++) {
		for (size_t j = 0; j < sizeof(frames) / sizeof(frames[0]); j++) {
			uint64_t k = frames[j];
			struct fake *f = fake_new(true, 1);
			push(f, false, at_frame(TICK, k, rates[r]));
			apply_scene_item_audio_actions(&f->item, f->buf, TICK, (size_t)rates[r]);
			check_span(f, 0, k, 1.0f);
			check_span(f, k, FRAMES, 0.0f);
			check_item(f, false, 0, 1, 0);
			fake_free(f);
		}
	}
}

/* An action on frame 1024 or later belongs to a later tick: it stays queued
 * and the buffer keeps the current visibility. */
static void action_past_the_tick(void **state)
{
	(void)state;
	struct fake *f = fake_new(true, 1);
	push(f, false, at_frame(TICK, FRAMES, 48000));
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, FRAMES, 1.0f);
	check_item(f, true, 1, 0, 1);
	assert_int_equal(f->item.audio_actions.array[0].timestamp, at_frame(TICK, FRAMES, 48000));
	fake_free(f);

	/* the last nanosecond of frame 1023 is still this tick */
	f = fake_new(true, 1);
	push(f, false, at_frame(TICK, FRAMES, 48000) - 1);
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, FRAMES - 1, 1.0f);
	check_span(f, FRAMES - 1, FRAMES, 0.0f);
	check_item(f, false, 0, 1, 0);
	fake_free(f);
}

/* An action timed before the tick (late) lands on frame 0. */
static void late_action_on_frame_0(void **state)
{
	(void)state;
	struct fake *f = fake_new(true, 1);
	push(f, false, TICK - 5 * NS);
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, FRAMES, 0.0f);
	check_item(f, false, 0, 1, 0);
	fake_free(f);

	/* timestamp 0 too */
	f = fake_new(false, 1);
	push(f, true, 0);
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, FRAMES, 1.0f);
	check_item(f, true, 1, 0, 0);
	fake_free(f);
}

/* Several toggles in one tick: each span has the visibility before its
 * change, and every hide is counted. */
static void several_changes_in_one_tick(void **state)
{
	(void)state;
	struct fake *f = fake_new(true, 2);
	push(f, false, at_frame(TICK, 100, 48000));
	push(f, true, at_frame(TICK, 300, 48000));
	push(f, false, at_frame(TICK, 700, 48000));
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, 100, 1.0f);
	check_span(f, 100, 300, 0.0f);
	check_span(f, 300, 700, 1.0f);
	check_span(f, 700, FRAMES, 0.0f);
	check_item(f, false, 0, 1, 0);
	fake_free(f);

	/* two changes on one frame: the span between them is empty */
	f = fake_new(true, 2);
	push(f, false, at_frame(TICK, 400, 48000));
	push(f, true, at_frame(TICK, 400, 48000));
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, FRAMES, 1.0f);
	check_item(f, true, 1, 0, 0);
	fake_free(f);

	/* on frame 0, then again later */
	f = fake_new(true, 2);
	push(f, false, TICK);
	push(f, true, at_frame(TICK, 1, 48000));
	push(f, false, at_frame(TICK, 1023, 48000));
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, 1, 0.0f);
	check_span(f, 1, 1023, 1.0f);
	check_span(f, 1023, FRAMES, 0.0f);
	check_item(f, false, 0, 1, 0);
	fake_free(f);
}

/* Actions in this tick are consumed; the first one past it and everything
 * after it stay queued, in order. */
static void later_actions_stay_queued(void **state)
{
	(void)state;
	struct fake *f = fake_new(true, 1);
	push(f, false, at_frame(TICK, 200, 48000));
	push(f, true, at_frame(TICK, 1500, 48000));
	push(f, false, at_frame(TICK, 3000, 48000));
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, 200, 1.0f);
	check_span(f, 200, FRAMES, 0.0f);
	check_item(f, false, 0, 1, 2);
	assert_true(f->item.audio_actions.array[0].visible);
	assert_int_equal(f->item.audio_actions.array[0].timestamp, at_frame(TICK, 1500, 48000));
	assert_false(f->item.audio_actions.array[1].visible);
	assert_int_equal(f->item.audio_actions.array[1].timestamp, at_frame(TICK, 3000, 48000));

	/* the next tick picks them up from its own start */
	uint64_t next = TICK + util_mul_div64(FRAMES, NS, 48000);
	apply_scene_item_audio_actions(&f->item, f->buf, next, 48000);
	size_t show = (size_t)util_mul_div64(at_frame(TICK, 1500, 48000) - next, 48000, NS);
	assert_int_equal(show, 1500 - FRAMES);
	check_span(f, 0, show, 0.0f);
	check_span(f, show, FRAMES, 1.0f);
	check_item(f, true, 0, 1, 1);
	fake_free(f);
}

/* Actions out of time order (two threads queuing at once).
 * Characterized, not endorsed: a change timed before the previous one does
 * not rewrite the buffer, so the hide below never reaches the gain buffer,
 * but it is counted. And the loop stops at the first action past the tick,
 * even when a later one is due. */
static void actions_out_of_order(void **state)
{
	(void)state;
	struct fake *f = fake_new(true, 1);
	push(f, false, at_frame(TICK, 600, 48000));
	push(f, true, at_frame(TICK, 200, 48000));
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, FRAMES, 1.0f);
	check_item(f, true, 0, 1, 0);
	fake_free(f);

	f = fake_new(true, 1);
	push(f, true, at_frame(TICK, 1500, 48000));
	push(f, false, at_frame(TICK, 100, 48000));
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, FRAMES, 1.0f);
	check_item(f, true, 1, 0, 2);
	fake_free(f);
}

/* The hide count: each applied hide drops one active reference, and the one
 * that reaches 0 removes the active child, once. Shows take no reference
 * here (obs_sceneitem_set_visible() takes it when it queues the show). */
static void hides_drop_references(void **state)
{
	(void)state;
	/* hide, show, hide, show, hide, from 3 references */
	struct fake *f = fake_new(true, 3);
	for (int i = 0; i < 5; i++)
		push(f, i % 2 != 0, at_frame(TICK, 100 + 100 * (uint64_t)i, 48000));
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_item(f, false, 0, 1, 0);
	fake_free(f);

	/* references to spare: no removal */
	f = fake_new(true, 5);
	push(f, false, at_frame(TICK, 10, 48000));
	push(f, true, at_frame(TICK, 20, 48000));
	push(f, false, at_frame(TICK, 30, 48000));
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_item(f, false, 3, 0, 0);
	fake_free(f);

	/* a hide of a hidden item still counts */
	f = fake_new(false, 2);
	push(f, false, at_frame(TICK, 10, 48000));
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_span(f, 0, FRAMES, 0.0f);
	check_item(f, false, 1, 0, 0);
	fake_free(f);

	/* Characterized, not endorsed: more hides than references go below 0
	 * and remove the child only at 0. obs_sceneitem_set_visible() keeps
	 * them paired, so libobs does not get here. */
	f = fake_new(true, 1);
	push(f, false, at_frame(TICK, 10, 48000));
	push(f, true, at_frame(TICK, 20, 48000));
	push(f, false, at_frame(TICK, 30, 48000));
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 48000);
	check_item(f, false, -1, 1, 0);
	fake_free(f);
}

/* ts 0 (the preview-only scene's video tick: no buffer, rate 0): every
 * action is applied, whatever its time, and the hides are counted. */
static void ts_0_applies_everything(void **state)
{
	(void)state;
	struct fake *f = fake_new(true, 2);
	push(f, false, TICK);
	push(f, true, TICK + 3600 * NS);
	push(f, false, UINT64_MAX);
	apply_scene_item_audio_actions(&f->item, NULL, 0, 0);
	check_item(f, false, 0, 1, 0);
	fake_free(f);

	/* with a rate (process_all_audio_actions() passes one) */
	f = fake_new(false, 1);
	push(f, true, TICK);
	push(f, false, TICK + 3600 * NS);
	push(f, true, UINT64_MAX);
	apply_scene_item_audio_actions(&f->item, NULL, 0, 192000);
	check_item(f, true, 0, 1, 0);
	fake_free(f);

	/* a tick with rate 0 puts every action on frame 0 (libobs passes rate 0
	 * only with ts 0) */
	f = fake_new(true, 1);
	push(f, false, TICK + 3600 * NS);
	apply_scene_item_audio_actions(&f->item, f->buf, TICK, 0);
	check_span(f, 0, FRAMES, 0.0f);
	check_item(f, false, 0, 1, 0);
	fake_free(f);
}

/* ------------------------------------------------------------------------ */
/* apply_scene_item_volume                                                   */

/* Nothing queued: false, and the buffer is not written (the mix then uses
 * the item's visibility, not the buffer). */
static void volume_without_actions(void **state)
{
	(void)state;
	struct fake *f = fake_new(true, 1);
	assert_false(apply_scene_item_volume(&f->item, f->buf, TICK, 48000));
	check_untouched(f);
	check_item(f, true, 1, 0, 0);
	fake_free(f);
}

/* The first action decides: due before ts + duration (1024 frames in ns,
 * rounded down), it applies the tick and returns true; later, false with
 * the buffer not written. */
static void volume_window(void **state)
{
	(void)state;
	static const uint64_t rates[] = {8000, 22050, 44100, 48000, 96000, 192000};

	for (size_t r = 0; r < sizeof(rates) / sizeof(rates[0]); r++) {
		uint64_t rate = rates[r];
		uint64_t duration = FRAMES * NS / rate;

		struct fake *f = fake_new(true, 1);
		push(f, false, TICK + duration - 1);
		assert_true(apply_scene_item_volume(&f->item, f->buf, TICK, (size_t)rate));
		check_span(f, 0, FRAMES - 1, 1.0f);
		check_span(f, FRAMES - 1, FRAMES, 0.0f);
		check_item(f, false, 0, 1, 0);
		fake_free(f);

		f = fake_new(true, 1);
		push(f, false, TICK + duration);
		assert_false(apply_scene_item_volume(&f->item, f->buf, TICK, (size_t)rate));
		check_untouched(f);
		check_item(f, true, 1, 0, 1);
		fake_free(f);
	}

	/* Characterized, not endorsed: where 1024 frames are not a whole number
	 * of nanoseconds, the rounded-down duration leaves out the end of the
	 * last frame. At 44.1 kHz an action at ts + duration is on frame 1023,
	 * but it waits for the next tick, which puts it on that tick's frame 0
	 * (one frame late). */
	struct fake *f = fake_new(true, 1);
	uint64_t duration = FRAMES * NS / 44100;
	assert_int_equal(util_mul_div64(duration, 44100, NS), FRAMES - 1);
	push(f, false, TICK + duration);
	assert_false(apply_scene_item_volume(&f->item, f->buf, TICK, 44100));
	check_item(f, true, 1, 0, 1);
	fake_free(f);
}

/* The window looks at the first action only (characterized, not endorsed:
 * with actions out of order, a due one waits behind one that is not). */
static void volume_first_action_only(void **state)
{
	(void)state;
	struct fake *f = fake_new(true, 1);
	push(f, true, at_frame(TICK, 5000, 48000));
	push(f, false, at_frame(TICK, 10, 48000));
	assert_false(apply_scene_item_volume(&f->item, f->buf, TICK, 48000));
	check_untouched(f);
	check_item(f, true, 1, 0, 2);
	fake_free(f);
}

/* ts 0: true whenever an action is queued, applied whatever its time. */
static void volume_ts_0(void **state)
{
	(void)state;
	struct fake *f = fake_new(true, 1);
	push(f, false, UINT64_MAX);
	assert_true(apply_scene_item_volume(&f->item, NULL, 0, 48000));
	check_item(f, false, 0, 1, 0);
	assert_false(apply_scene_item_volume(&f->item, NULL, 0, 48000));
	fake_free(f);
}

/* ------------------------------------------------------------------------ */
/* process_all_audio_actions                                                 */

/* No audio playing in the scene: everything queued is applied now, future
 * actions included, and every hide is counted. */
static void process_all(void **state)
{
	(void)state;
	struct fake *f = fake_new(true, 2);
	push(f, false, TICK);
	push(f, true, TICK + NS);
	push(f, false, TICK + 3600 * NS);
	process_all_audio_actions(&f->item, 48000);
	check_item(f, false, 0, 1, 0);
	fake_free(f);

	f = fake_new(false, 1);
	process_all_audio_actions(&f->item, 48000);
	check_item(f, false, 1, 0, 0);
	fake_free(f);
}

int main(void)
{
	base_set_log_handler(log_handler, NULL);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(no_actions),
		cmocka_unit_test(one_change_on_each_frame),
		cmocka_unit_test(one_change_at_each_rate),
		cmocka_unit_test(action_past_the_tick),
		cmocka_unit_test(late_action_on_frame_0),
		cmocka_unit_test(several_changes_in_one_tick),
		cmocka_unit_test(later_actions_stay_queued),
		cmocka_unit_test(actions_out_of_order),
		cmocka_unit_test(hides_drop_references),
		cmocka_unit_test(ts_0_applies_everything),
		cmocka_unit_test(volume_without_actions),
		cmocka_unit_test(volume_window),
		cmocka_unit_test(volume_first_action_only),
		cmocka_unit_test(volume_ts_0),
		cmocka_unit_test(process_all),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

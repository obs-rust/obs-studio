/*
 * Characterization of the rest of libobs/obs-source-transition.c
 * (obs-rust/obs-studio#4092), before it is ported: obs_transition_audio_render()
 * with mix_child(), transition_process_audio(), transition_live_sources() and
 * transition_calc_min_ts() it uses, and obs_transition_init().
 *
 * The source_transition_render, source_transition_sample_time and
 * source_transition_clock parity tests pin the rules on plain values, and
 * test_transition_audio_clock, test_transition_removed_child_audio and
 * test_group_hide_transition_audio run the audio through the real mix
 * thread. These cases call obs_transition_audio_render() directly, with no
 * audio subsystem running, on a transition whose children's audio timestamp,
 * pending and duplicated flags and output buffers the test sets (through
 * obs-internal.h), with mix callbacks that count their calls and record the
 * time each sample gets:
 * - idle, slot A's whole audio block is copied into the mix's (every mix and
 *   channel, whatever the mixers and channels), and no callback runs; the
 *   result is slot A's timestamp;
 * - no timestamp (no child, or each pending or removed): false, a timestamp
 *   of 0 and nothing written;
 * - transitioning, each child that is there and valid is mixed with its
 *   callback into the mixes the mixer mask selects and the channels asked
 *   for, out[i] += in[i] * mix(data, t), on top of what the mix held; a
 *   pending or duplicated child is skipped (a pending one gives no
 *   timestamp); the earliest timestamp of the children is the result;
 * - the place of a child's audio: from the frame its timestamp is past the
 *   earliest (a partial block), nothing for a start past the block, and
 *   nothing for a timestamp of 0;
 * - the time each sample gets: the manual value, or in auto mode the clock
 *   at the child's timestamp plus the sample's offset;
 * - the audio stop: at t >= 1 with the video ended, transitioning_audio ends
 *   and the transition stops (slot B moves to slot A, the stop signal, after
 *   the lock), and the new slot A is copied; with the video running only the
 *   audio ends; with no timestamp and the video ended it stops too; a stop
 *   that leaves a removed child in slot A gives no timestamp (#2185);
 * - obs_transition_init(): a new transition is aligned left and top, has
 *   both texrenders and counts slot A as active.
 *
 * Transitions take core video for their render targets, so the test only
 * builds on Linux and runs under xvfb-run; without a display the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <obs-internal.h>
#include <util/base.h>
#include <util/platform.h>
#include <util/threading.h>

#define CHILD_ID "test_tr_audio_child"
#define TRANSITION_ID "test_tr_audio_transition"
#define RATE 48000
#define FRAMES AUDIO_OUTPUT_FRAMES
#define BLOCK_FLOATS (MAX_AUDIO_MIXES * MAX_AUDIO_CHANNELS * AUDIO_OUTPUT_FRAMES)
/* One second, the first child's timestamp. */
#define T0 1000000000ULL
/* 512 frames at 48 kHz, rounded up: ns_to_audio_frames() gives 512. */
#define HALF_BLOCK_NS 10666667ULL
/* 1100 frames at 48 kHz: past the block. */
#define LATE_NS 22916667ULL

static bool obs_up;

static const char *child_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "audio child";
}

static void *child_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void child_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static struct obs_source_info child_info = {
	.id = CHILD_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = child_name,
	.create = child_create,
	.destroy = child_destroy,
};

static const char *tr_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "audio transition";
}

static void *tr_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void tr_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

/* Registration requires one; nothing outputs these transitions and no audio
 * subsystem runs, so nothing calls it. */
static bool tr_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
			    size_t channels, size_t sample_rate)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(audio);
	UNUSED_PARAMETER(mixers);
	UNUSED_PARAMETER(channels);
	UNUSED_PARAMETER(sample_rate);
	*ts_out = 0;
	return false;
}

static struct obs_source_info transition_info = {
	.id = TRANSITION_ID,
	.type = OBS_SOURCE_TYPE_TRANSITION,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = tr_name,
	.create = tr_create,
	.destroy = tr_destroy,
	.audio_render = tr_audio_render,
};

/* What one mix callback saw: its calls, the data, and the time of each of its
 * first FRAMES calls. */
struct mix_rec {
	long calls;
	void *data;
	float t[FRAMES];
};

static struct mix_rec rec_a;
static struct mix_rec rec_b;

static void record(struct mix_rec *rec, void *data, float t)
{
	if (rec->calls < FRAMES)
		rec->t[rec->calls] = t;
	rec->calls++;
	rec->data = data;
}

static float mix_a(void *data, float t)
{
	record(&rec_a, data, t);
	return 0.25f;
}

static float mix_b(void *data, float t)
{
	record(&rec_b, data, t);
	return 0.5f;
}

/* The sources of the running case, released by case_teardown if a failed
 * assertion ended the case first. */
static struct {
	obs_source_t *tr;
	obs_source_t *child[2];
} live;

static long stop_signals;

/* The mix the render writes into, one block as libobs's own. */
static float out_block[BLOCK_FLOATS];
static float expect_block[BLOCK_FLOATS];
static struct obs_source_audio_mix out;

static void on_stop(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	assert_ptr_equal(calldata_ptr(cd, "source"), live.tr);
	stop_signals++;
}

static size_t at(size_t mix, size_t ch, size_t frame)
{
	return (mix * MAX_AUDIO_CHANNELS + ch) * FRAMES + frame;
}

static int case_setup(void **state)
{
	UNUSED_PARAMETER(state);
	memset(&rec_a, 0, sizeof(rec_a));
	memset(&rec_b, 0, sizeof(rec_b));
	stop_signals = 0;
	for (size_t i = 0; i < BLOCK_FLOATS; i++)
		out_block[i] = 0.125f;
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
			out.output[mix].data[ch] = &out_block[at(mix, ch, 0)];
	memcpy(expect_block, out_block, sizeof(out_block));
	return 0;
}

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (live.tr) {
		signal_handler_t *sh = obs_source_get_signal_handler(live.tr);
		signal_handler_disconnect(sh, "transition_stop", on_stop, NULL);
		obs_transition_clear(live.tr);
	}
	obs_source_release(live.tr);
	for (size_t i = 0; i < 2; i++)
		obs_source_release(live.child[i]);
	memset(&live, 0, sizeof(live));
	return 0;
}

static void skip_without_core(void)
{
	if (!obs_up)
		skip();
}

/* Child i's audio: a value of its own for each mix, channel and frame. */
static float child_value(size_t i, size_t mix, size_t ch, size_t frame)
{
	return (float)(i + 1) + (float)mix * 0.5f + (float)ch * 0.0625f + (float)frame / 1024.0f;
}

static void set_child(size_t i, uint64_t ts, bool pending, bool duplicated)
{
	obs_source_t *child = live.child[i];
	child->audio_ts = ts;
	child->audio_pending = pending;
	child->audio_is_duplicated = duplicated;
}

/* A transition holding child 0 in slot A, child 1 to start towards, its stop
 * signal counted. Both children have their audio filled and are valid at
 * T0. */
static obs_source_t *make_transition(const char *name)
{
	for (size_t i = 0; i < 2; i++) {
		char buf[64];
		snprintf(buf, sizeof(buf), "%s %c", name, (char)('A' + i));
		live.child[i] = obs_source_create_private(CHILD_ID, buf, NULL);
		assert_non_null(live.child[i]);
		for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
			for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
				for (size_t f = 0; f < FRAMES; f++)
					live.child[i]->audio_output_buf[mix][ch][f] = child_value(i, mix, ch, f);
		set_child(i, T0, false, false);
	}
	live.tr = obs_source_create(TRANSITION_ID, name, NULL, NULL);
	assert_non_null(live.tr);
	signal_handler_t *sh = obs_source_get_signal_handler(live.tr);
	signal_handler_connect(sh, "transition_stop", on_stop, NULL);
	obs_transition_set(live.tr, live.child[0]);
	return live.tr;
}

/* Set the clock under the transition's lock. The video tick snaps the manual
 * value to its target, so both are set. */
static void set_clock(obs_source_t *tr, enum obs_transition_mode mode, float manual, uint64_t start, uint64_t duration)
{
	pthread_mutex_lock(&tr->transition_mutex);
	tr->transition_mode = mode;
	tr->transition_manual_torque = 0.0f;
	tr->transition_manual_target = manual;
	tr->transition_manual_val = manual;
	tr->transition_start_time = start;
	tr->transition_duration = duration;
	pthread_mutex_unlock(&tr->transition_mutex);
}

static void set_flags(obs_source_t *tr, bool video, bool audio)
{
	pthread_mutex_lock(&tr->transition_mutex);
	os_atomic_set_bool(&tr->transitioning_video, video);
	os_atomic_set_bool(&tr->transitioning_audio, audio);
	pthread_mutex_unlock(&tr->transition_mutex);
}

/* A manual transition from child 0 to child 1, held at `t`. */
static void start_at(obs_source_t *tr, float t)
{
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, live.child[1]));
	set_clock(tr, OBS_TRANSITION_MODE_MANUAL, t, 0, 0);
}

static bool render(obs_source_t *tr, uint64_t *ts, uint32_t mixers, size_t channels)
{
	*ts = 12345;
	return obs_transition_audio_render(tr, ts, &out, mixers, channels, RATE, mix_a, mix_b);
}

/* Add child i's audio times `mul` into the expected mix, from frame `pos`. */
static void expect_mix(size_t i, float mul, uint32_t mixers, size_t channels, size_t pos)
{
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		if ((mixers & (1u << mix)) == 0)
			continue;
		for (size_t ch = 0; ch < channels; ch++)
			for (size_t f = pos; f < FRAMES; f++)
				expect_block[at(mix, ch, f)] += child_value(i, mix, ch, f - pos) * mul;
	}
}

static void expect_copy(size_t i)
{
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
			for (size_t f = 0; f < FRAMES; f++)
				expect_block[at(mix, ch, f)] = child_value(i, mix, ch, f);
}

static void check_block(void)
{
	for (size_t i = 0; i < BLOCK_FLOATS; i++) {
		if (out_block[i] != expect_block[i]) {
			fprintf(stderr, "mix %zu channel %zu frame %zu: %f, expected %f\n",
				i / (MAX_AUDIO_CHANNELS * FRAMES), (i / FRAMES) % MAX_AUDIO_CHANNELS, i % FRAMES,
				(double)out_block[i], (double)expect_block[i]);
			fail();
		}
	}
}

static void check_slots(obs_source_t *tr, obs_source_t *a, obs_source_t *b, bool active)
{
	obs_source_t *got = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_A);
	assert_ptr_equal(got, a);
	obs_source_release(got);
	got = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_B);
	assert_ptr_equal(got, b);
	obs_source_release(got);
	assert_true(obs_transition_is_active(tr) == active);
}

/* Idle, slot A's whole block is copied (whatever the mixers and channels),
 * no callback runs, and the result is slot A's timestamp. */
static void idle_copies_a(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("idle");
	uint64_t ts;

	assert_true(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, T0);
	expect_copy(0);
	check_block();
	assert_int_equal(rec_a.calls, 0);
	assert_int_equal(rec_b.calls, 0);
	check_slots(tr, live.child[0], NULL, false);
}

/* No child, or each pending, or removed: false, 0 and nothing written. */
static void no_timestamp_writes_nothing(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("no timestamp");
	uint64_t ts;

	set_child(0, T0, true, false);
	assert_false(render(tr, &ts, 0x3F, 2));
	assert_int_equal(ts, 0);
	check_block();

	set_child(0, T0, false, false);
	obs_source_remove(live.child[0]);
	assert_false(render(tr, &ts, 0x3F, 2));
	assert_int_equal(ts, 0);
	check_block();

	obs_transition_clear(tr);
	assert_false(render(tr, &ts, 0x3F, 2));
	assert_int_equal(ts, 0);
	check_block();
	assert_int_equal(rec_a.calls, 0);
	assert_int_equal(rec_b.calls, 0);
	assert_int_equal(stop_signals, 0);
}

/* A slot A child with a timestamp of 0 that is not pending gives no
 * timestamp either: false, 0 and nothing copied. */
static void zero_timestamp_is_none(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("zero");
	uint64_t ts;

	set_child(0, 0, false, false);
	assert_false(render(tr, &ts, 0x3F, 2));
	assert_int_equal(ts, 0);
	check_block();
}

/* Transitioning, both children mix with their callbacks into the selected
 * mixes and asked channels, on top of the mix; the data is the transition's
 * and every sample gets the manual value. */
static void transitioning_mixes_both(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("mix");
	uint64_t ts;
	start_at(tr, 0.5f);

	assert_true(render(tr, &ts, 0x5, 2));
	assert_int_equal(ts, T0);
	expect_mix(0, 0.25f, 0x5, 2, 0);
	expect_mix(1, 0.5f, 0x5, 2, 0);
	check_block();
	assert_int_equal(rec_a.calls, 2 * 2 * FRAMES);
	assert_int_equal(rec_b.calls, 2 * 2 * FRAMES);
	assert_ptr_equal(rec_a.data, tr);
	assert_ptr_equal(rec_b.data, tr);
	for (size_t i = 0; i < FRAMES; i++) {
		assert_true(rec_a.t[i] == 0.5f);
		assert_true(rec_b.t[i] == 0.5f);
	}
	check_slots(tr, live.child[0], live.child[1], true);
	assert_int_equal(stop_signals, 0);
}

/* No mixer selected: nothing mixed, the timestamp still returned. */
static void no_mixers_mix_nothing(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("no mixers");
	uint64_t ts;
	start_at(tr, 0.5f);

	assert_true(render(tr, &ts, 0, 2));
	assert_int_equal(ts, T0);
	check_block();
	assert_int_equal(rec_a.calls, 0);
	assert_int_equal(rec_b.calls, 0);
}

/* A pending child is skipped and gives no timestamp; a duplicated one is
 * skipped but its timestamp counts. */
static void pending_and_duplicated_are_skipped(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("skip");
	uint64_t ts;
	start_at(tr, 0.5f);

	set_child(0, T0 - 1000, true, false);
	assert_true(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, T0);
	expect_mix(1, 0.5f, 0x1, 1, 0);
	check_block();
	assert_int_equal(rec_a.calls, 0);
	assert_int_equal(rec_b.calls, FRAMES);

	set_child(0, T0, false, false);
	set_child(1, T0 - HALF_BLOCK_NS, false, true);
	assert_true(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, T0 - HALF_BLOCK_NS);
	expect_mix(0, 0.25f, 0x1, 1, 512);
	check_block();
	assert_int_equal(rec_a.calls, FRAMES - 512);
	assert_int_equal(rec_b.calls, FRAMES);
}

/* A child later than the earliest mixes from its frame on; one past the
 * block mixes nothing; one with a timestamp of 0 mixes nothing. */
static void place_follows_the_timestamp(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("place");
	uint64_t ts;
	start_at(tr, 0.5f);

	set_child(1, T0 + HALF_BLOCK_NS, false, false);
	assert_true(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, T0);
	expect_mix(0, 0.25f, 0x1, 1, 0);
	expect_mix(1, 0.5f, 0x1, 1, 512);
	check_block();
	assert_int_equal(rec_a.calls, FRAMES);
	assert_int_equal(rec_b.calls, FRAMES - 512);

	set_child(1, T0 + LATE_NS, false, false);
	assert_true(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, T0);
	expect_mix(0, 0.25f, 0x1, 1, 0);
	check_block();
	assert_int_equal(rec_a.calls, 2 * FRAMES);
	assert_int_equal(rec_b.calls, FRAMES - 512);

	set_child(1, 0, false, false);
	assert_true(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, T0);
	expect_mix(0, 0.25f, 0x1, 1, 0);
	check_block();
	assert_int_equal(rec_a.calls, 3 * FRAMES);
	assert_int_equal(rec_b.calls, FRAMES - 512);
}

/* In auto mode each sample gets the clock at the child's timestamp plus the
 * sample's offset. */
static void auto_mode_times_each_sample(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("auto");
	uint64_t ts;
	start_at(tr, 0.0f);
	/* A start at T0 lasting 2048 frames' worth. */
	set_clock(tr, OBS_TRANSITION_MODE_AUTO, 0.0f, T0, 2 * 2 * HALF_BLOCK_NS);
	set_child(1, T0 + HALF_BLOCK_NS, false, false);

	assert_true(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, T0);
	assert_int_equal(rec_a.calls, FRAMES);
	assert_int_equal(rec_b.calls, FRAMES - 512);
	assert_true(rec_a.t[0] == 0.0f);
	for (size_t i = 1; i < FRAMES; i++)
		assert_true(rec_a.t[i] > rec_a.t[i - 1]);
	assert_true(rec_a.t[FRAMES - 1] < 0.5f);
	/* B's samples run on its own timestamp: its first is a quarter in, as
	 * A's sample 512 is. */
	assert_true(rec_b.t[0] > 0.249f && rec_b.t[0] < 0.251f);
	assert_true(rec_a.t[512] > 0.249f && rec_a.t[512] < 0.251f);
	for (size_t i = 1; i < FRAMES - 512; i++)
		assert_true(rec_b.t[i] > rec_b.t[i - 1]);
	check_slots(tr, live.child[0], live.child[1], true);
}

/* At t >= 1 with the video ended, the audio ends, the transition stops after
 * the lock (slot B moves to slot A) and the new slot A is copied. */
static void audio_end_stops(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("stop");
	uint64_t ts;
	start_at(tr, 1.0f);
	set_flags(tr, false, true);
	set_child(1, T0 + 1000, false, false);

	assert_true(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, T0 + 1000);
	assert_int_equal(stop_signals, 1);
	check_slots(tr, live.child[1], NULL, false);
	assert_false(tr->transitioning_audio);
	assert_false(tr->transitioning_video);
	expect_copy(1);
	check_block();
	assert_int_equal(rec_a.calls, 0);
	assert_int_equal(rec_b.calls, 0);
}

/* At t >= 1 with the video running, only the audio ends: nothing stops, and
 * slot A is copied. */
static void audio_end_with_video_running(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("end");
	uint64_t ts;
	start_at(tr, 1.0f);

	assert_true(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, T0);
	assert_int_equal(stop_signals, 0);
	check_slots(tr, live.child[0], live.child[1], true);
	assert_false(tr->transitioning_audio);
	assert_true(tr->transitioning_video);
	expect_copy(0);
	check_block();
	assert_int_equal(rec_a.calls, 0);
	assert_int_equal(rec_b.calls, 0);

	/* Below 1 nothing ends. */
	set_flags(tr, true, true);
	set_clock(tr, OBS_TRANSITION_MODE_MANUAL, 0.99f, 0, 0);
	assert_true(render(tr, &ts, 0x1, 1));
	assert_true(tr->transitioning_audio);
	assert_int_equal(stop_signals, 0);
}

/* With no timestamp and the video ended, the audio ends and the transition
 * stops; with the video running it keeps. */
static void no_timestamp_stop(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("no timestamp stop");
	uint64_t ts;
	start_at(tr, 0.5f);
	set_child(0, T0, true, false);
	set_child(1, T0, true, false);

	assert_false(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, 0);
	assert_true(tr->transitioning_audio);
	assert_int_equal(stop_signals, 0);

	set_flags(tr, false, true);
	assert_false(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, 0);
	assert_false(tr->transitioning_audio);
	assert_int_equal(stop_signals, 1);
	check_slots(tr, live.child[1], NULL, false);
	check_block();
}

/* A removed child B is left out; when the stop makes it slot A, the render
 * gives no timestamp (#2185). */
static void removed_child_after_stop(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("removed");
	uint64_t ts;
	start_at(tr, 0.5f);
	obs_source_remove(live.child[1]);

	assert_true(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, T0);
	expect_mix(0, 0.25f, 0x1, 1, 0);
	check_block();
	assert_int_equal(rec_a.calls, FRAMES);
	assert_int_equal(rec_b.calls, 0);

	set_clock(tr, OBS_TRANSITION_MODE_MANUAL, 1.0f, 0, 0);
	set_flags(tr, false, true);
	assert_false(render(tr, &ts, 0x1, 1));
	assert_int_equal(ts, 0);
	assert_int_equal(stop_signals, 1);
	check_slots(tr, live.child[1], NULL, false);
	check_block();
	assert_int_equal(rec_a.calls, FRAMES);
}

/* A new transition is aligned left and top, has both texrenders and counts
 * slot A as active. */
static void init_state(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("init");

	assert_int_equal(obs_transition_get_alignment(tr), OBS_ALIGN_LEFT | OBS_ALIGN_TOP);
	assert_non_null(tr->transition_texrender[0]);
	assert_non_null(tr->transition_texrender[1]);
	assert_ptr_not_equal(tr->transition_texrender[0], tr->transition_texrender[1]);
	assert_true(tr->transition_source_active[0]);
	assert_false(tr->transition_source_active[1]);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
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

	obs_register_source(&child_info);
	obs_register_source(&transition_info);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(idle_copies_a, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(no_timestamp_writes_nothing, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(zero_timestamp_is_none, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(transitioning_mixes_both, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(no_mixers_mix_nothing, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(pending_and_duplicated_are_skipped, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(place_follows_the_timestamp, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(auto_mode_times_each_sample, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(audio_end_stops, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(audio_end_with_video_running, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(no_timestamp_stop, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(removed_child_after_stop, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(init_state, case_setup, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

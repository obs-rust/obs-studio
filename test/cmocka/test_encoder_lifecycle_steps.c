#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-encoder-lifecycle.h"
#include "obs-encoder-audio-start.h"
#include "obs-encoder-bind.h"

/*
 * Characterization of the encoder callback and connection lifecycle decisions
 * of libobs/obs-encoder.c, moved to libobs/obs-encoder-lifecycle.c before their
 * Rust port:
 * - obs_encoder_start_internal(): the first callback (_first) and whether the
 *   callback is already there (_found); the keyframe request for a video
 *   encoder that gains a callback asks obs_encoder_audio_start_not_video();
 * - obs_encoder_stop(): the callback found (_found) and the last one (_last),
 *   and the group destroy (_group_idle after destroy_on_stop);
 * - add_connection() and remove_connection(): the group counters (_lcm_unset,
 *   _started_inc, _group_ready, _started_dec, _group_idle); the route asks
 *   obs_encoder_bind_not_audio(), the audio setter's refusal;
 * - gpu_encode_available(): the texture cap (_pass_texture);
 * - send_first_video_packet(): the SEI in front of the first keyframe
 *   (_prepend_sei); send_packet()'s first packet route asks
 *   obs_encoder_audio_start_not_video().
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The bodies of obs_encoder_start_internal(), obs_encoder_stop(),
 * add_connection(), remove_connection(), gpu_encode_available(), full_stop(),
 * send_packet() and send_first_video_packet() are also copied below onto a
 * stand-in encoder (no lock, no video or audio output, no darray; the mix,
 * the outputs, the group and the plugin's SEI are given, and every call the C
 * makes is counted), so every decision is pinned end to end for audio and
 * video encoders, with and without a group, a mix and a texture cap.
 *
 * The functions are hidden in libobs, so this test compiles the C sources into
 * itself and calls only them, through their headers. The real-libobs tests
 * test_encoder_join_keyframe (the keyframe request), test_encoder_error_stop
 * (the full stop and the texture teardown), test_encoder_group_null,
 * test_multitrack_ladder (groups), test_output_interleave_e2e,
 * test_output_interleave_restart, test_texture_encoder_raw_format and
 * test_output_delay_restart cover the callers.
 *
 * Nothing here comes from outside OBS: the counts and indexes come from the
 * encoder's own lists, the caps from the encoder plugin, the SEI from the
 * encoder plugin.
 */

#define TYPE_AUDIO 0
#define TYPE_VIDEO 1
#define CAP_PASS_TEXTURE 2u

static const int edge_type[] = {INT_MIN, -1, TYPE_AUDIO, TYPE_VIDEO, 2, 3, INT_MAX};
#define N_EDGE_TYPE (sizeof(edge_type) / sizeof(edge_type[0]))
static const size_t edge_size[] = {0, 1, 2, 3, 255, 0x7fffffff, SIZE_MAX - 1, SIZE_MAX};
#define N_EDGE_SIZE (sizeof(edge_size) / sizeof(edge_size[0]))
static const uint32_t edge_u32[] = {0, 1, 2, 3, 6, 0x7fffffffU, 0x80000000U, UINT32_MAX - 1, UINT32_MAX};
#define N_EDGE_U32 (sizeof(edge_u32) / sizeof(edge_u32[0]))

/* ------------------------------------------------------------------------ */
/* the constants and the decisions */

static void constants(void **state)
{
	(void)state;
	/* OBS_ENCODER_CAP_PASS_TEXTURE, DARRAY_INVALID; the type tests'
	 * OBS_ENCODER_AUDIO and OBS_ENCODER_VIDEO */
	assert_int_equal(OBS_ENCODER_BIND_TYPE_AUDIO, 0);
	assert_int_equal(OBS_ENCODER_LIFECYCLE_CAP_PASS_TEXTURE, 2);
	assert_true(OBS_ENCODER_LIFECYCLE_NOT_FOUND == SIZE_MAX);
	assert_int_equal(OBS_ENCODER_AUDIO_START_TYPE_VIDEO, 1);
}

static void first_found_last(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_SIZE; i++) {
		const size_t n = edge_size[i];
		/* first and last: no callback (before the add; after the erase) */
		assert_int_equal(obs_encoder_lifecycle_first(n), n == 0);
		assert_int_equal(obs_encoder_lifecycle_last(n), n == 0);
		/* found: any index but DARRAY_INVALID */
		assert_int_equal(obs_encoder_lifecycle_found(n), n != SIZE_MAX);
	}
	assert_false(obs_encoder_lifecycle_found(OBS_ENCODER_LIFECYCLE_NOT_FOUND));
	assert_true(obs_encoder_lifecycle_found(0));
	assert_true(obs_encoder_lifecycle_found(SIZE_MAX - 1));
}

static void routes(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_TYPE; i++) {
		const int t = edge_type[i];
		/* the audio mix only for an audio encoder; an unknown type takes the
		 * video route, as the C's else does */
		assert_int_equal(!obs_encoder_bind_not_audio(t), t == TYPE_AUDIO);
		/* the keyframe request and the first packet route: video only */
		assert_int_equal(!obs_encoder_audio_start_not_video(t), t == TYPE_VIDEO);
	}
	assert_true(obs_encoder_bind_not_audio(2));
	assert_true(obs_encoder_audio_start_not_video(2));
}

static void pass_texture(void **state)
{
	(void)state;
	for (uint32_t caps = 0; caps < 256; caps++) {
		assert_int_equal(obs_encoder_lifecycle_pass_texture(caps), (caps & CAP_PASS_TEXTURE) != 0);
		assert_int_equal(obs_encoder_lifecycle_pass_texture(caps | 0xffff0000U),
				 (caps & CAP_PASS_TEXTURE) != 0);
	}
	assert_false(obs_encoder_lifecycle_pass_texture(0));
	assert_false(obs_encoder_lifecycle_pass_texture(1)); /* OBS_ENCODER_CAP_DEPRECATED */
	assert_true(obs_encoder_lifecycle_pass_texture(CAP_PASS_TEXTURE));
	assert_true(obs_encoder_lifecycle_pass_texture(UINT32_MAX));
	assert_false(obs_encoder_lifecycle_pass_texture(UINT32_MAX & ~CAP_PASS_TEXTURE));
}

static void group_counters(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_U32; i++) {
		const uint32_t v = edge_u32[i];
		assert_int_equal(obs_encoder_lifecycle_lcm_unset(v), v == 0);
		assert_int_equal(obs_encoder_lifecycle_group_idle(v), v == 0);
		assert_int_equal(obs_encoder_lifecycle_started_inc(v), (uint32_t)(v + 1u));
		assert_int_equal(obs_encoder_lifecycle_started_dec(v), (uint32_t)(v - 1u));
		for (size_t j = 0; j < N_EDGE_SIZE; j++)
			assert_int_equal(obs_encoder_lifecycle_group_ready(v, edge_size[j]), (size_t)v >= edge_size[j]);
	}
	/* the counters wrap as the C's uint32_t does (characterized, not
	 * endorsed: a stop with none started leaves UINT32_MAX, so the group is
	 * never idle again) */
	assert_int_equal(obs_encoder_lifecycle_started_inc(UINT32_MAX), 0);
	assert_int_equal(obs_encoder_lifecycle_started_dec(0), UINT32_MAX);
	/* ready: every encoder started, or more; a group of none is ready */
	assert_false(obs_encoder_lifecycle_group_ready(1, 2));
	assert_true(obs_encoder_lifecycle_group_ready(2, 2));
	assert_true(obs_encoder_lifecycle_group_ready(3, 2));
	assert_true(obs_encoder_lifecycle_group_ready(0, 0));
}

static void prepend_sei(void **state)
{
	(void)state;
	for (int got = 0; got < 2; got++) {
		for (int has = 0; has < 2; has++) {
			for (size_t i = 0; i < N_EDGE_SIZE; i++) {
				const size_t size = edge_size[i];
				assert_int_equal(obs_encoder_lifecycle_prepend_sei(got, has, size),
						 got && has && size != 0);
			}
		}
	}
	/* a plugin that sets data but returns false: sent as it is */
	assert_false(obs_encoder_lifecycle_prepend_sei(false, true, 3));
	/* data with size 0, or a size with no data: sent as it is */
	assert_false(obs_encoder_lifecycle_prepend_sei(true, true, 0));
	assert_false(obs_encoder_lifecycle_prepend_sei(true, false, 5));
	assert_true(obs_encoder_lifecycle_prepend_sei(true, true, 1));
}

/* ------------------------------------------------------------------------ */
/* the stand-in encoder and the callers */

#define MAX_CBS 4
#define MAX_OUTPUTS 3

struct stand_cb {
	int fn;
	int param;
	bool sent_first_packet;
};

struct stand_group {
	bool destroy_on_stop;
	size_t num_encoders;
	uint32_t started;
	uint32_t lcm;
	int resets;
	int ready_adds;
	int destroyed;
};

struct stand_output {
	bool active;
	int force_stops;
	int null_packets;
};

/* the plugin's get_sei_data() */
enum sei_mode {
	SEI_NONE,      /* no get_sei_data */
	SEI_FALSE,     /* returns false, sets nothing */
	SEI_NULL,      /* returns true, data NULL, size 5 */
	SEI_EMPTY,     /* returns true, data set, size 0 */
	SEI_DATA,      /* returns true, 3 bytes */
	SEI_FALSE_SET, /* returns false with 3 bytes set */
};

struct stand_encoder {
	int type;
	uint32_t caps;
	bool has_data;
	bool has_media;
	/* get_mix_for_video(): a mix, and its texture flags */
	bool has_mix;
	bool nv12;
	bool p010;
	bool has_request_keyframe;
	uint32_t divisor;
	enum sei_mode sei;
	struct stand_cb cbs[MAX_CBS];
	size_t num_cbs;
	struct stand_group *group;
	struct stand_output outputs[MAX_OUTPUTS];
	size_t num_outputs;
	bool initialized;
	bool active;
	bool gpu_encode_failed;
	size_t packet_times;
	/* counted side effects */
	int audio_connects;
	int audio_disconnects;
	int gpu_starts;
	int gpu_stops;
	int raw_starts;
	int raw_stops;
	int shutdowns;
	int keyframe_requests;
	int pause_resets;
	int mix_asks;
	int sei_asks;
	/* the packets the callbacks got: the size and whether the SEI led */
	int packets;
	size_t last_size;
	bool last_with_sei;
};

static void stand_init(struct stand_encoder *e, int type)
{
	*e = (struct stand_encoder){0};
	e->type = type;
	e->has_data = true;
	e->has_media = true;
	e->has_mix = true;
	e->nv12 = true;
	e->divisor = 1;
}

/* lcm() and gcd() of obs-encoder.c, which stay C */
static int stand_gcd(int a, int b)
{
	return b == 0 ? a : stand_gcd(b, a % b);
}

static int stand_lcm(int a, int b)
{
	return (a / stand_gcd(a, b)) * b;
}

/* gpu_encode_available() */
static bool stand_gpu_available(struct stand_encoder *e)
{
	e->mix_asks++;
	if (!e->has_media || !e->has_mix)
		return false;
	return obs_encoder_lifecycle_pass_texture(e->caps) && (e->p010 || e->nv12);
}

/* add_connection() */
static void stand_add_connection(struct stand_encoder *e)
{
	if (!obs_encoder_bind_not_audio(e->type)) {
		e->audio_connects++;
	} else {
		if (stand_gpu_available(e))
			e->gpu_starts++;
		else
			e->raw_starts++;
	}

	if (e->group) {
		struct stand_group *g = e->group;
		if (obs_encoder_lifecycle_lcm_unset(g->lcm))
			g->lcm = 1;
		g->lcm = (uint32_t)stand_lcm((int)g->lcm, (int)e->divisor);

		uint32_t started = obs_encoder_lifecycle_started_inc(g->started);
		g->started = started;
		bool ready = obs_encoder_lifecycle_group_ready(started, g->num_encoders);
		if (ready)
			g->ready_adds++;
	}

	e->active = true;
}

/* remove_connection() */
static void stand_remove_connection(struct stand_encoder *e, bool shutdown)
{
	if (!obs_encoder_bind_not_audio(e->type)) {
		e->audio_disconnects++;
	} else {
		if (stand_gpu_available(e))
			e->gpu_stops++;
		else
			e->raw_stops++;
	}

	if (e->group) {
		struct stand_group *g = e->group;
		uint32_t started = obs_encoder_lifecycle_started_dec(g->started);
		g->started = started;
		if (obs_encoder_lifecycle_group_idle(started)) {
			g->resets++;
			g->lcm = 1;
		}
	}

	if (shutdown)
		e->shutdowns++;
	e->initialized = false;
	e->active = false;
}

static size_t stand_callback_idx(const struct stand_encoder *e, int fn, int param)
{
	for (size_t i = 0; i < e->num_cbs; i++)
		if (e->cbs[i].fn == fn && e->cbs[i].param == param)
			return i;
	return OBS_ENCODER_LIFECYCLE_NOT_FOUND;
}

/* obs_encoder_start_internal() */
static void stand_start(struct stand_encoder *e, int fn, int param)
{
	bool first = false;

	if (!e->has_data || !e->has_media)
		return;

	first = obs_encoder_lifecycle_first(e->num_cbs);

	size_t idx = stand_callback_idx(e, fn, param);
	bool added = !obs_encoder_lifecycle_found(idx);
	if (added && e->num_cbs < MAX_CBS)
		e->cbs[e->num_cbs++] = (struct stand_cb){fn, param, false};

	if (first) {
		e->pause_resets++;
		stand_add_connection(e);
	} else if (added && !obs_encoder_audio_start_not_video(e->type) && e->has_request_keyframe) {
		e->keyframe_requests++;
	}
}

/* obs_encoder_stop() after the checks */
static void stand_stop(struct stand_encoder *e, int fn, int param)
{
	bool last = false;
	size_t idx = stand_callback_idx(e, fn, param);
	if (obs_encoder_lifecycle_found(idx)) {
		for (size_t i = idx; i + 1 < e->num_cbs; i++)
			e->cbs[i] = e->cbs[i + 1];
		e->num_cbs--;
		last = obs_encoder_lifecycle_last(e->num_cbs);
	}

	e->packet_times = 0;

	if (last) {
		stand_remove_connection(e, true);
		struct stand_group *g = e->group;
		if (g && g->destroy_on_stop && obs_encoder_lifecycle_group_idle(g->started))
			g->destroyed++;
	}
}

/* full_stop() with an encoder */
static void stand_full_stop(struct stand_encoder *e)
{
	bool gpu_encode = stand_gpu_available(e);
	if (gpu_encode)
		e->gpu_encode_failed = true;

	for (size_t i = 0; i < e->num_outputs; i++) {
		struct stand_output *o = &e->outputs[i];
		if (!o->active)
			continue;
		o->force_stops++;
		o->null_packets++;
	}

	if (gpu_encode)
		return;

	e->num_cbs = 0;
	stand_remove_connection(e, false);
}

/* get_sei() and the plugin */
static bool stand_get_sei(struct stand_encoder *e, bool *has, size_t *size)
{
	if (e->sei == SEI_NONE)
		return false;
	e->sei_asks++;
	switch (e->sei) {
	case SEI_FALSE:
		return false;
	case SEI_NULL:
		*has = false;
		*size = 5;
		return true;
	case SEI_EMPTY:
		*has = true;
		*size = 0;
		return true;
	case SEI_DATA:
		*has = true;
		*size = 3;
		return true;
	case SEI_FALSE_SET:
		*has = true;
		*size = 3;
		return false;
	default:
		return false;
	}
}

static void stand_deliver(struct stand_encoder *e, struct stand_cb *cb, size_t size, bool with_sei)
{
	(void)cb;
	e->packets++;
	e->last_size = size;
	e->last_with_sei = with_sei;
}

/* send_first_video_packet() */
static void stand_send_first(struct stand_encoder *e, struct stand_cb *cb, bool keyframe, size_t size)
{
	bool has = false;
	size_t sei_size = 0;

	if (!keyframe)
		return;

	bool got = stand_get_sei(e, &has, &sei_size);
	if (!obs_encoder_lifecycle_prepend_sei(got, has, sei_size)) {
		stand_deliver(e, cb, size, false);
		cb->sent_first_packet = true;
		return;
	}

	stand_deliver(e, cb, sei_size + size, true);
	cb->sent_first_packet = true;
}

/* send_packet() */
static void stand_send(struct stand_encoder *e, size_t cb_idx, bool keyframe, size_t size)
{
	struct stand_cb *cb = &e->cbs[cb_idx];
	if (!obs_encoder_audio_start_not_video(e->type) && !cb->sent_first_packet)
		stand_send_first(e, cb, keyframe, size);
	else
		stand_deliver(e, cb, size, false);
}

/* ------------------------------------------------------------------------ */
/* end to end */

static void start_callbacks(void **state)
{
	(void)state;
	struct stand_encoder e;

	for (int t = 0; t < 2; t++) {
		const int type = t ? TYPE_VIDEO : TYPE_AUDIO;
		for (int kf = 0; kf < 2; kf++) {
			stand_init(&e, type);
			e.has_request_keyframe = kf;

			/* 0 callbacks: the first connects, no keyframe request */
			stand_start(&e, 1, 1);
			assert_int_equal(e.num_cbs, 1);
			assert_true(e.active);
			assert_int_equal(e.pause_resets, 1);
			assert_int_equal(e.audio_connects + e.gpu_starts + e.raw_starts, 1);
			assert_int_equal(e.keyframe_requests, 0);

			/* 1 callback: a second output joins; only a video encoder
			 * with request_keyframe asks for a keyframe */
			stand_start(&e, 1, 2);
			assert_int_equal(e.num_cbs, 2);
			assert_int_equal(e.pause_resets, 1);
			assert_int_equal(e.audio_connects + e.gpu_starts + e.raw_starts, 1);
			assert_int_equal(e.keyframe_requests, type == TYPE_VIDEO && kf);

			/* 2 callbacks: a third */
			stand_start(&e, 2, 1);
			assert_int_equal(e.num_cbs, 3);
			assert_int_equal(e.keyframe_requests, type == TYPE_VIDEO && kf ? 2 : 0);

			/* a duplicate: not added, no keyframe request */
			stand_start(&e, 1, 2);
			assert_int_equal(e.num_cbs, 3);
			assert_int_equal(e.keyframe_requests, type == TYPE_VIDEO && kf ? 2 : 0);
			assert_int_equal(e.audio_connects + e.gpu_starts + e.raw_starts, 1);
		}
	}

	/* an unknown type: the video route, but no keyframe request */
	stand_init(&e, 2);
	e.has_request_keyframe = true;
	stand_start(&e, 1, 1);
	stand_start(&e, 1, 2);
	assert_int_equal(e.raw_starts + e.gpu_starts, 1);
	assert_int_equal(e.keyframe_requests, 0);

	/* no data or no media: nothing at all */
	for (int i = 0; i < 2; i++) {
		stand_init(&e, TYPE_VIDEO);
		if (i)
			e.has_media = false;
		else
			e.has_data = false;
		stand_start(&e, 1, 1);
		assert_int_equal(e.num_cbs, 0);
		assert_false(e.active);
		assert_int_equal(e.pause_resets, 0);
		assert_int_equal(e.mix_asks, 0);
	}
}

static void stop_callbacks(void **state)
{
	(void)state;
	struct stand_encoder e;

	stand_init(&e, TYPE_VIDEO);
	stand_start(&e, 1, 1);
	stand_start(&e, 1, 2);
	e.initialized = true;
	e.packet_times = 4;

	/* a missing callback: nothing erased, still connected; the packet times
	 * are cleared anyway (characterized, not endorsed) */
	stand_stop(&e, 2, 2);
	assert_int_equal(e.num_cbs, 2);
	assert_true(e.active);
	assert_int_equal(e.packet_times, 0);
	assert_int_equal(e.raw_stops + e.gpu_stops, 0);

	/* not the last: erased, still connected */
	e.packet_times = 2;
	stand_stop(&e, 1, 1);
	assert_int_equal(e.num_cbs, 1);
	assert_int_equal(e.cbs[0].param, 2);
	assert_true(e.active);
	assert_true(e.initialized);
	assert_int_equal(e.packet_times, 0);

	/* the last: disconnected and shut down */
	stand_stop(&e, 1, 2);
	assert_int_equal(e.num_cbs, 0);
	assert_false(e.active);
	assert_false(e.initialized);
	assert_int_equal(e.gpu_stops + e.raw_stops, 1);
	assert_int_equal(e.shutdowns, 1);

	/* stopping again: missing, nothing more */
	stand_stop(&e, 1, 2);
	assert_int_equal(e.gpu_stops + e.raw_stops, 1);
	assert_int_equal(e.shutdowns, 1);

	/* an audio encoder disconnects from its mix */
	stand_init(&e, TYPE_AUDIO);
	stand_start(&e, 3, 0);
	stand_stop(&e, 3, 0);
	assert_int_equal(e.audio_connects, 1);
	assert_int_equal(e.audio_disconnects, 1);
	assert_int_equal(e.mix_asks, 0);
}

static void connect_routes(void **state)
{
	(void)state;
	struct stand_encoder e;

	/* audio: the mix, no GPU test */
	stand_init(&e, TYPE_AUDIO);
	e.caps = CAP_PASS_TEXTURE;
	stand_add_connection(&e);
	assert_int_equal(e.audio_connects, 1);
	assert_int_equal(e.mix_asks, 0);

	/* video: GPU with the cap and an NV12 or P010 mix, else raw */
	for (int cap = 0; cap < 2; cap++) {
		for (int mix = 0; mix < 2; mix++) {
			for (int nv12 = 0; nv12 < 2; nv12++) {
				for (int p010 = 0; p010 < 2; p010++) {
					stand_init(&e, TYPE_VIDEO);
					e.caps = cap ? CAP_PASS_TEXTURE | 1u : 1u;
					e.has_mix = mix;
					e.nv12 = nv12;
					e.p010 = p010;
					const bool gpu = cap && mix && (nv12 || p010);
					stand_add_connection(&e);
					assert_int_equal(e.gpu_starts, gpu);
					assert_int_equal(e.raw_starts, !gpu);
					stand_remove_connection(&e, false);
					assert_int_equal(e.gpu_stops, gpu);
					assert_int_equal(e.raw_stops, !gpu);
					assert_int_equal(e.shutdowns, 0);
					assert_int_equal(e.mix_asks, 2);
				}
			}
		}
	}

	/* the route is asked again at the stop: a mix that lost its NV12
	 * textures in between stops raw what it started on the GPU
	 * (characterized, not endorsed) */
	stand_init(&e, TYPE_VIDEO);
	e.caps = CAP_PASS_TEXTURE;
	stand_add_connection(&e);
	e.nv12 = false;
	stand_remove_connection(&e, true);
	assert_int_equal(e.gpu_starts, 1);
	assert_int_equal(e.raw_stops, 1);
	assert_int_equal(e.gpu_stops, 0);
	assert_int_equal(e.shutdowns, 1);
}

static void group_lifecycle(void **state)
{
	(void)state;
	struct stand_encoder a, b;
	struct stand_group g;

	for (int destroy = 0; destroy < 2; destroy++) {
		g = (struct stand_group){0};
		g.destroy_on_stop = destroy;
		g.num_encoders = 2;
		stand_init(&a, TYPE_VIDEO);
		stand_init(&b, TYPE_VIDEO);
		a.group = &g;
		b.group = &g;
		a.divisor = 2;
		b.divisor = 3;

		/* 0 started: the LCM starts at 1, then takes the divisor */
		stand_start(&a, 1, 1);
		assert_int_equal(g.started, 1);
		assert_int_equal(g.lcm, 2);
		assert_int_equal(g.ready_adds, 0);

		/* the encoder count: ready */
		stand_start(&b, 1, 1);
		assert_int_equal(g.started, 2);
		assert_int_equal(g.lcm, 6);
		assert_int_equal(g.ready_adds, 1);

		/* back to 1: no reset, no destroy */
		stand_stop(&a, 1, 1);
		assert_int_equal(g.started, 1);
		assert_int_equal(g.resets, 0);
		assert_int_equal(g.destroyed, 0);
		assert_int_equal(g.lcm, 6);

		/* back to 0: reset, destroyed only with destroy_on_stop */
		stand_stop(&b, 1, 1);
		assert_int_equal(g.started, 0);
		assert_int_equal(g.resets, 1);
		assert_int_equal(g.lcm, 1);
		assert_int_equal(g.destroyed, destroy);
	}

	/* a group of one is ready at its first start; an LCM already set is not
	 * reset to 1 */
	g = (struct stand_group){0};
	g.num_encoders = 1;
	g.lcm = 4;
	stand_init(&a, TYPE_AUDIO);
	a.group = &g;
	a.divisor = 6;
	stand_start(&a, 1, 1);
	assert_int_equal(g.ready_adds, 1);
	assert_int_equal(g.lcm, 12);

	/* a divisor of 0 (the setter refuses it) gives an LCM of 0, which the
	 * next start takes as unset (characterized, not endorsed) */
	g = (struct stand_group){0};
	g.num_encoders = 3;
	stand_init(&a, TYPE_VIDEO);
	stand_init(&b, TYPE_VIDEO);
	a.group = &g;
	b.group = &g;
	a.divisor = 0;
	b.divisor = 2;
	stand_start(&a, 1, 1);
	assert_int_equal(g.lcm, 0);
	stand_start(&b, 1, 1);
	assert_int_equal(g.lcm, 2);
	assert_int_equal(g.ready_adds, 0);

	/* a stop with none started wraps the count (characterized, not
	 * endorsed): no reset and no destroy */
	g = (struct stand_group){0};
	g.destroy_on_stop = true;
	g.num_encoders = 1;
	stand_init(&a, TYPE_VIDEO);
	a.group = &g;
	stand_remove_connection(&a, false);
	assert_int_equal(g.started, UINT32_MAX);
	assert_int_equal(g.resets, 0);
}

static void full_stop_outputs(void **state)
{
	(void)state;
	struct stand_encoder e;

	/* a raw encoder: only the active outputs are force stopped (our fix:
	 * force stopping an idle output leaves its stopping event unsignalled),
	 * then the callbacks go and it disconnects without a shutdown */
	stand_init(&e, TYPE_VIDEO);
	e.caps = 0;
	stand_start(&e, 1, 1);
	stand_start(&e, 1, 2);
	e.num_outputs = 3;
	e.outputs[0].active = true;
	e.outputs[1].active = false;
	e.outputs[2].active = true;
	stand_full_stop(&e);
	assert_int_equal(e.outputs[0].force_stops, 1);
	assert_int_equal(e.outputs[1].force_stops, 0);
	assert_int_equal(e.outputs[2].force_stops, 1);
	assert_int_equal(e.outputs[0].null_packets, 1);
	assert_int_equal(e.outputs[1].null_packets, 0);
	assert_int_equal(e.num_cbs, 0);
	assert_false(e.active);
	assert_int_equal(e.raw_stops, 1);
	assert_int_equal(e.shutdowns, 0);
	assert_false(e.gpu_encode_failed);

	/* a texture encoder: flagged, outputs stopped, then the early return
	 * (the outputs stop it when they end capture) */
	stand_init(&e, TYPE_VIDEO);
	e.caps = CAP_PASS_TEXTURE;
	stand_start(&e, 1, 1);
	e.num_outputs = 2;
	e.outputs[0].active = true;
	e.outputs[1].active = true;
	stand_full_stop(&e);
	assert_true(e.gpu_encode_failed);
	assert_int_equal(e.outputs[0].force_stops, 1);
	assert_int_equal(e.outputs[1].force_stops, 1);
	assert_int_equal(e.num_cbs, 1);
	assert_true(e.active);
	assert_int_equal(e.gpu_stops, 0);

	/* an audio encoder with no outputs: disconnects */
	stand_init(&e, TYPE_AUDIO);
	stand_start(&e, 1, 1);
	stand_full_stop(&e);
	assert_int_equal(e.audio_disconnects, 1);
	assert_int_equal(e.num_cbs, 0);
	assert_false(e.gpu_encode_failed);
}

static void first_packets(void **state)
{
	(void)state;
	struct stand_encoder e;

	static const struct {
		enum sei_mode sei;
		bool with_sei;
	} modes[] = {
		{SEI_NONE, false},  {SEI_FALSE, false}, {SEI_NULL, false},
		{SEI_EMPTY, false}, {SEI_DATA, true},   {SEI_FALSE_SET, false},
	};

	for (size_t m = 0; m < sizeof(modes) / sizeof(modes[0]); m++) {
		stand_init(&e, TYPE_VIDEO);
		e.sei = modes[m].sei;
		stand_start(&e, 1, 1);

		/* not a keyframe: dropped, still waiting */
		stand_send(&e, 0, false, 10);
		assert_int_equal(e.packets, 0);
		assert_false(e.cbs[0].sent_first_packet);
		assert_int_equal(e.sei_asks, 0);

		/* the first keyframe: SEI in front only with data and a size */
		stand_send(&e, 0, true, 10);
		assert_int_equal(e.packets, 1);
		assert_true(e.cbs[0].sent_first_packet);
		assert_int_equal(e.last_with_sei, modes[m].with_sei);
		assert_int_equal(e.last_size, modes[m].with_sei ? 13 : 10);
		assert_int_equal(e.sei_asks, modes[m].sei != SEI_NONE);

		/* after it: plain, keyframe or not, no more SEI */
		stand_send(&e, 0, false, 7);
		assert_int_equal(e.packets, 2);
		assert_false(e.last_with_sei);
		assert_int_equal(e.last_size, 7);
		assert_int_equal(e.sei_asks, modes[m].sei != SEI_NONE);

		/* a second output joins: it waits for its own first keyframe */
		stand_start(&e, 1, 2);
		stand_send(&e, 1, false, 5);
		assert_int_equal(e.packets, 2);
		stand_send(&e, 1, true, 5);
		assert_int_equal(e.packets, 3);
		assert_int_equal(e.last_with_sei, modes[m].with_sei);
	}

	/* an audio encoder: every packet plain, keyframe or not */
	stand_init(&e, TYPE_AUDIO);
	e.sei = SEI_DATA;
	stand_start(&e, 1, 1);
	stand_send(&e, 0, false, 4);
	assert_int_equal(e.packets, 1);
	assert_false(e.last_with_sei);
	assert_false(e.cbs[0].sent_first_packet);
	assert_int_equal(e.sei_asks, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(first_found_last),
		cmocka_unit_test(routes),
		cmocka_unit_test(pass_texture),
		cmocka_unit_test(group_counters),
		cmocka_unit_test(prepend_sei),
		cmocka_unit_test(start_callbacks),
		cmocka_unit_test(stop_callbacks),
		cmocka_unit_test(connect_routes),
		cmocka_unit_test(group_lifecycle),
		cmocka_unit_test(full_stop_outputs),
		cmocka_unit_test(first_packets),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}

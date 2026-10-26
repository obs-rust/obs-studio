#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-output-slots.h"

/*
 * Characterization of the encoder slot rules and setter gates of
 * libobs/obs-output.c, moved to libobs/obs-output-slots.c before their Rust
 * port:
 * - get_first_video_encoder_index() / get_first_audio_encoder_index(): the
 *   lowest occupied slot of a mask (obs_output_slots_first_video, _audio);
 * - obs_output_set_mixer(): the range check and the mask
 *   (_mixer_in_range, _mixer_mask);
 * - the type gates of obs_output_set_video_encoder2() and
 *   obs_output_set_audio_encoder(), also the slot choice of
 *   obs_output_remove_encoder_internal() (_video_type_ok, _audio_type_ok);
 * - the index rules of the two setters (_video_index_ok, _audio_index_ok);
 * - the default index and the preferred size step (_default_index,
 *   _scaled_axis_set);
 * - the getters' and obs_output_set_preferred_size2()'s bounds
 *   (_video_slot, _audio_slot).
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The callers (the two encoder setters, the getters, obs_output_audio(),
 * obs_output_set_mixer(), obs_output_remove_encoder_internal() and
 * obs_output_set_preferred_size2()) are also copied below onto a stand-in
 * output that records every warning, active load, reference, output
 * hookup, caption track and scaled size in order, so the gates are pinned
 * end to end for raw, encoded, video-only, audio-only and multitrack
 * outputs: indexes 0, 1, max - 1, max and SIZE_MAX, NULL encoders (clearing
 * a slot destroys its caption track), the wrong encoder type, active
 * outputs, the same encoder twice, the preferred size with and without a
 * scaled size, and set_mixer at MAX_AUDIO_MIXES - 1 and at it.
 * obs_output_set_service(), obs_output_set_mixers() and
 * obs_output_remove_encoder() make no decision of their own beyond the
 * flag, active and NULL checks obs-output.c keeps, so nothing of theirs
 * moved.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header. The real-libobs
 * tests (test_multitrack_encoder_override, test_multitrack_ladder,
 * test_output_input_checks, test_service_encoder_settings,
 * test_output_interleave_e2e, test_output_caption_codec,
 * test_raw_output_video_reset, test_output_null_id) cover the callers.
 *
 * Nothing here comes from outside OBS: the flags come from an output
 * plugin, the encoder's type from an encoder plugin, the indexes and sizes
 * from the frontend or a plugin.
 */

#define MAXV OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS
#define MAXA OBS_OUTPUT_SLOTS_MAX_AUDIO_ENCODERS
#define MIXES OBS_OUTPUT_SLOTS_MAX_AUDIO_MIXES
#define MT_AUDIO OBS_OUTPUT_SLOTS_MULTI_TRACK_AUDIO
#define MT_VIDEO OBS_OUTPUT_SLOTS_MULTI_TRACK_VIDEO
#define T_AUDIO OBS_OUTPUT_SLOTS_ENCODER_AUDIO
#define T_VIDEO OBS_OUTPUT_SLOTS_ENCODER_VIDEO

/* obs-output.h's OBS_OUTPUT_VIDEO, _AUDIO, _ENCODED */
#define F_VIDEO (1 << 0)
#define F_AUDIO (1 << 1)
#define F_ENCODED (1 << 2)

static const size_t edge_idx[] = {0, 1, 2, 5, 6, 7, 9, 10, 11, 31, 32, 63, 64, SIZE_MAX - 1, SIZE_MAX};
#define N_EDGE_IDX (sizeof(edge_idx) / sizeof(edge_idx[0]))

static void constants(void **state)
{
	(void)state;
	assert_int_equal(OBS_OUTPUT_SLOTS_MULTI_TRACK_AUDIO, 16);
	assert_int_equal(OBS_OUTPUT_SLOTS_MULTI_TRACK_VIDEO, 64);
	assert_int_equal(OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS, 10);
	assert_int_equal(OBS_OUTPUT_SLOTS_MAX_AUDIO_ENCODERS, 6);
	assert_int_equal(OBS_OUTPUT_SLOTS_MAX_AUDIO_MIXES, 6);
	assert_int_equal(OBS_OUTPUT_SLOTS_ENCODER_AUDIO, 0);
	assert_int_equal(OBS_OUTPUT_SLOTS_ENCODER_VIDEO, 1);
}

/* ------------------------------------------------------------------------ */
/* get_first_*_encoder_index() */

static size_t lowest_bit(uint32_t mask, size_t below)
{
	for (size_t i = 0; i < below; i++) {
		if (mask & ((uint32_t)1 << i))
			return i;
	}
	return below;
}

static void first_video(void **state)
{
	(void)state;
	static const uint32_t high[] = {0, 0x400, 0xfffffc00, 0x80000000};
	for (size_t h = 0; h < sizeof(high) / sizeof(high[0]); h++) {
		for (uint32_t m = 0; m < 0x400; m++) {
			uint32_t mask = m | high[h];
			size_t idx = 777;
			bool found = obs_output_slots_first_video(mask, &idx);
			size_t want = lowest_bit(mask, MAXV);
			assert_int_equal(found, want < MAXV);
			assert_int_equal(idx, found ? want : 777);
		}
	}
	size_t idx = 777;
	assert_true(obs_output_slots_first_video(1u << 9, &idx));
	assert_int_equal(idx, 9);
	/* only slots below 10 count; nothing is written when none is set */
	idx = 777;
	assert_false(obs_output_slots_first_video(1u << 10, &idx));
	assert_false(obs_output_slots_first_video(0, &idx));
	assert_false(obs_output_slots_first_video(0xfffffc00, &idx));
	assert_int_equal(idx, 777);
}

static void first_audio(void **state)
{
	(void)state;
	static const uint32_t high[] = {0, 0x40, 0xffffffc0, 0x80000000};
	for (size_t h = 0; h < sizeof(high) / sizeof(high[0]); h++) {
		for (uint32_t m = 0; m < 0x40; m++) {
			uint32_t mask = m | high[h];
			size_t idx = 777;
			bool found = obs_output_slots_first_audio(mask, &idx);
			size_t want = lowest_bit(mask, MAXA);
			assert_int_equal(found, want < MAXA);
			assert_int_equal(idx, found ? want : 777);
		}
	}
	size_t idx = 777;
	assert_true(obs_output_slots_first_audio(1u << 5, &idx));
	assert_int_equal(idx, 5);
	/* slots 6 to 9 are video only */
	idx = 777;
	assert_false(obs_output_slots_first_audio(1u << 6, &idx));
	assert_false(obs_output_slots_first_audio(0x3c0, &idx));
	assert_int_equal(idx, 777);
}

/* ------------------------------------------------------------------------ */
/* obs_output_set_mixer() */

static void mixer(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_IDX; i++)
		assert_int_equal(obs_output_slots_mixer_in_range(edge_idx[i]), edge_idx[i] < MIXES);
	assert_true(obs_output_slots_mixer_in_range(MIXES - 1));
	assert_false(obs_output_slots_mixer_in_range(MIXES));
	for (size_t i = 0; i < MIXES; i++)
		assert_int_equal(obs_output_slots_mixer_mask(i), (size_t)1 << i);
	/* the shift itself, past the mixes (set_mixer never asks there) */
	for (size_t i = 0; i < sizeof(size_t) * 8; i++)
		assert_int_equal(obs_output_slots_mixer_mask(i), (size_t)1 << i);
}

/* ------------------------------------------------------------------------ */
/* the type gates */

static void type_gates(void **state)
{
	(void)state;
	static const int types[] = {-2, -1, 0, 1, 2, 3, 9, 0x7fffffff};
	for (size_t i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
		assert_int_equal(obs_output_slots_video_type_ok(types[i]), types[i] == 1);
		assert_int_equal(obs_output_slots_audio_type_ok(types[i]), types[i] == 0);
	}
	assert_true(obs_output_slots_video_type_ok(T_VIDEO));
	assert_false(obs_output_slots_video_type_ok(T_AUDIO));
	assert_true(obs_output_slots_audio_type_ok(T_AUDIO));
	assert_false(obs_output_slots_audio_type_ok(T_VIDEO));
}

/* ------------------------------------------------------------------------ */
/* the index rules */

static void index_rules(void **state)
{
	(void)state;
	for (uint32_t f = 0; f < 0x100; f++) {
		for (size_t i = 0; i < N_EDGE_IDX; i++) {
			size_t idx = edge_idx[i];
			bool v = (f & MT_VIDEO) ? idx < MAXV : idx == 0;
			bool a = (f & MT_AUDIO) ? idx < MAXA : idx == 0;
			assert_int_equal(obs_output_slots_video_index_ok(f, idx), v);
			assert_int_equal(obs_output_slots_audio_index_ok(f, idx), a);
			/* high flag bits change nothing */
			assert_int_equal(obs_output_slots_video_index_ok(f | 0xffffff00, idx), v);
			assert_int_equal(obs_output_slots_audio_index_ok(f | 0xffffff00, idx), a);
		}
	}
	/* multitrack audio (OBS_OUTPUT_MULTI_TRACK) says nothing about video */
	assert_false(obs_output_slots_video_index_ok(MT_AUDIO, 1));
	assert_true(obs_output_slots_video_index_ok(MT_VIDEO, MAXV - 1));
	assert_false(obs_output_slots_video_index_ok(MT_VIDEO, MAXV));
	assert_false(obs_output_slots_audio_index_ok(MT_VIDEO, 1));
	assert_true(obs_output_slots_audio_index_ok(MT_AUDIO, MAXA - 1));
	assert_false(obs_output_slots_audio_index_ok(MT_AUDIO, MAXA));
	assert_true(obs_output_slots_video_index_ok(0, 0));
	assert_true(obs_output_slots_audio_index_ok(0, 0));
	assert_false(obs_output_slots_video_index_ok(0, SIZE_MAX));
	assert_false(obs_output_slots_audio_index_ok(MT_AUDIO, SIZE_MAX));
}

static void default_and_bounds(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_IDX; i++) {
		size_t idx = edge_idx[i];
		assert_int_equal(obs_output_slots_default_index(idx), idx == 0);
		assert_int_equal(obs_output_slots_video_slot(idx), idx < MAXV);
		assert_int_equal(obs_output_slots_audio_slot(idx), idx < MAXA);
	}
	static const uint32_t sizes[] = {0, 1, 2, 720, 1280, 0x80000000, UINT32_MAX};
	for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); i++)
		assert_int_equal(obs_output_slots_scaled_axis_set(sizes[i]), sizes[i] != 0);
}

/* ------------------------------------------------------------------------ */
/* the callers, copied from obs-output.c onto a stand-in output */

struct si_encoder {
	int id;
	int type;
	/* obs_encoder_get_ref() fails (the encoder is being destroyed) */
	bool dying;
};

struct stand_in {
	uint32_t flags;
	bool active;
	struct si_encoder *video[MAXV];
	struct si_encoder *audio[MAXA];
	int captions[MAXV];
	uint32_t scaled_width, scaled_height;
	size_t mixer_mask;
	int next_caption;
	char ev[512];
};

static void si_ev(struct stand_in *o, const char *fmt, ...)
{
	char buf[64];
	va_list args;
	va_start(args, fmt);
	vsnprintf(buf, sizeof(buf), fmt, args);
	va_end(args);
	if (o->ev[0])
		strncat(o->ev, " ", sizeof(o->ev) - strlen(o->ev) - 1);
	strncat(o->ev, buf, sizeof(o->ev) - strlen(o->ev) - 1);
}

static int si_id(const struct si_encoder *e)
{
	return e ? e->id : 0;
}

static bool si_log_flag(struct stand_in *o, uint32_t flag, bool inverse_log)
{
	bool ret = (o->flags & flag) != 0;
	if ((!inverse_log && !ret) || (inverse_log && ret))
		si_ev(o, "warn-flag");
	return ret;
}

static bool si_active(struct stand_in *o)
{
	si_ev(o, "active?");
	return o->active;
}

static struct si_encoder *si_get_ref(struct stand_in *o, struct si_encoder *e)
{
	si_ev(o, "ref%d", si_id(e));
	return e && !e->dying ? e : NULL;
}

static void si_release(struct stand_in *o, struct si_encoder *e)
{
	si_ev(o, "release%d", si_id(e));
}

static void si_destroy_caption(struct stand_in *o, size_t idx)
{
	if (o->captions[idx]) {
		si_ev(o, "uncaption%zu", idx);
		o->captions[idx] = 0;
	}
}

static uint32_t si_video_mask(const struct stand_in *o)
{
	uint32_t occupied = 0;
	for (size_t i = 0; i < MAXV; i++) {
		if (o->video[i])
			occupied |= (uint32_t)1 << i;
	}
	return occupied;
}

static uint32_t si_audio_mask(const struct stand_in *o)
{
	uint32_t occupied = 0;
	for (size_t i = 0; i < MAXA; i++) {
		if (o->audio[i])
			occupied |= (uint32_t)1 << i;
	}
	return occupied;
}

static void si_set_video_encoder2(struct stand_in *o, struct si_encoder *encoder, size_t idx)
{
	if (!si_log_flag(o, F_ENCODED, false) || !si_log_flag(o, F_VIDEO, false))
		return;
	if (encoder && !obs_output_slots_video_type_ok(encoder->type)) {
		si_ev(o, "warn-type");
		return;
	}
	if (si_active(o)) {
		si_ev(o, "warn-active");
		return;
	}
	if (!obs_output_slots_video_index_ok(o->flags, idx))
		return;
	if (o->video[idx] == encoder)
		return;

	si_ev(o, "unhook%d", si_id(o->video[idx]));
	si_release(o, o->video[idx]);
	o->video[idx] = si_get_ref(o, encoder);
	si_ev(o, "hook%d", si_id(o->video[idx]));

	si_destroy_caption(o, idx);
	if (encoder != NULL) {
		o->captions[idx] = ++o->next_caption;
		si_ev(o, "caption%zu", idx);
	} else {
		o->captions[idx] = 0;
	}

	if (obs_output_slots_default_index(idx)) {
		if (obs_output_slots_scaled_axis_set(o->scaled_width) &&
		    obs_output_slots_scaled_axis_set(o->scaled_height))
			si_ev(o, "size%d=%ux%u", si_id(o->video[idx]), o->scaled_width, o->scaled_height);
	}
}

static void si_set_audio_encoder(struct stand_in *o, struct si_encoder *encoder, size_t idx)
{
	if (!si_log_flag(o, F_ENCODED, false) || !si_log_flag(o, F_AUDIO, false))
		return;
	if (encoder && !obs_output_slots_audio_type_ok(encoder->type)) {
		si_ev(o, "warn-type");
		return;
	}
	if (si_active(o)) {
		si_ev(o, "warn-active");
		return;
	}
	if (!obs_output_slots_audio_index_ok(o->flags, idx))
		return;
	if (o->audio[idx] == encoder)
		return;

	si_ev(o, "unhook%d", si_id(o->audio[idx]));
	si_release(o, o->audio[idx]);
	o->audio[idx] = si_get_ref(o, encoder);
	si_ev(o, "hook%d", si_id(o->audio[idx]));
}

static int si_get_video_encoder2(const struct stand_in *o, size_t idx)
{
	if (!obs_output_slots_video_slot(idx))
		return -1;
	return si_id(o->video[idx]);
}

static int si_get_audio_encoder(const struct stand_in *o, size_t idx)
{
	if (!obs_output_slots_audio_slot(idx))
		return -1;
	return si_id(o->audio[idx]);
}

static int si_get_video_encoder(const struct stand_in *o)
{
	size_t idx;
	if (obs_output_slots_first_video(si_video_mask(o), &idx))
		return si_get_video_encoder2(o, idx);
	return -1;
}

/* obs_output_audio(): its own audio (100) for a raw output, else the first
 * audio encoder's (its id), else none (-1) */
static int si_audio(const struct stand_in *o)
{
	if ((o->flags & F_ENCODED) == 0)
		return 100;
	size_t idx;
	if (obs_output_slots_first_audio(si_audio_mask(o), &idx))
		return si_id(o->audio[idx]);
	return -1;
}

static void si_set_mixer(struct stand_in *o, size_t mixer_idx)
{
	if (si_log_flag(o, F_ENCODED, true))
		return;
	if (!obs_output_slots_mixer_in_range(mixer_idx)) {
		si_ev(o, "warn-mixer");
		return;
	}
	if (si_active(o))
		return;
	o->mixer_mask = obs_output_slots_mixer_mask(mixer_idx);
}

static void si_remove_encoder_internal(struct stand_in *o, struct si_encoder *encoder)
{
	if (obs_output_slots_video_type_ok(encoder->type)) {
		for (size_t i = 0; i < MAXV; i++) {
			if (o->video[i] == encoder) {
				o->video[i] = NULL;
				si_release(o, encoder);
			}
		}
	} else if (obs_output_slots_audio_type_ok(encoder->type)) {
		for (size_t i = 0; i < MAXA; i++) {
			if (o->audio[i] == encoder) {
				o->audio[i] = NULL;
				si_release(o, encoder);
			}
		}
	}
}

static void si_set_preferred_size2(struct stand_in *o, uint32_t width, uint32_t height, size_t idx)
{
	if (!si_log_flag(o, F_VIDEO, false))
		return;
	if (!obs_output_slots_video_slot(idx))
		return;
	if (si_active(o)) {
		si_ev(o, "warn-active");
		return;
	}
	if (obs_output_slots_default_index(idx)) {
		o->scaled_width = width;
		o->scaled_height = height;
	}
	if (o->flags & F_ENCODED) {
		if (o->video[idx])
			si_ev(o, "size%d=%ux%u", si_id(o->video[idx]), width, height);
	}
}

static struct si_encoder V1 = {1, T_VIDEO, false}, V2 = {2, T_VIDEO, false}, A3 = {3, T_AUDIO, false},
			 A4 = {4, T_AUDIO, false}, X5 = {5, 2, false}, D6 = {6, T_VIDEO, true};

static struct stand_in si_new(uint32_t flags)
{
	struct stand_in o;
	memset(&o, 0, sizeof(o));
	o.flags = flags;
	return o;
}

static void si_clear(struct stand_in *o)
{
	o->ev[0] = 0;
}

static void e2e_video_flags(void **state)
{
	(void)state;
	/* a raw output: refused with the encoded warning, active not loaded */
	struct stand_in o = si_new(F_VIDEO | F_AUDIO);
	si_set_video_encoder2(&o, &V1, 0);
	assert_string_equal(o.ev, "warn-flag");
	assert_null(o.video[0]);
	/* an encoded audio-only output: the video warning */
	o = si_new(F_ENCODED | F_AUDIO);
	si_set_video_encoder2(&o, &V1, 0);
	assert_string_equal(o.ev, "warn-flag");
	/* an audio encoder: the type warning, active not loaded */
	o = si_new(F_ENCODED | F_VIDEO);
	o.active = true;
	si_set_video_encoder2(&o, &A3, 0);
	assert_string_equal(o.ev, "warn-type");
	si_clear(&o);
	si_set_video_encoder2(&o, &X5, 0);
	assert_string_equal(o.ev, "warn-type");
	/* active: refused after the type gate, even with NULL */
	si_clear(&o);
	si_set_video_encoder2(&o, &V1, 0);
	assert_string_equal(o.ev, "active? warn-active");
	si_clear(&o);
	si_set_video_encoder2(&o, NULL, 5);
	assert_string_equal(o.ev, "active? warn-active");
	assert_null(o.video[0]);
}

static void e2e_video_index(void **state)
{
	(void)state;
	/* single track: index 0 only, others refused silently */
	struct stand_in o = si_new(F_ENCODED | F_VIDEO);
	static const size_t refused[] = {1, MAXV - 1, MAXV, SIZE_MAX};
	for (size_t i = 0; i < 4; i++) {
		si_clear(&o);
		si_set_video_encoder2(&o, &V1, refused[i]);
		assert_string_equal(o.ev, "active?");
	}
	si_clear(&o);
	si_set_video_encoder2(&o, &V1, 0);
	assert_string_equal(o.ev, "active? unhook0 release0 ref1 hook1 caption0");
	assert_ptr_equal(o.video[0], &V1);
	assert_int_equal(o.captions[0], 1);

	/* multitrack video: below MAX_OUTPUT_VIDEO_ENCODERS */
	o = si_new(F_ENCODED | F_VIDEO | MT_VIDEO);
	si_set_video_encoder2(&o, &V2, MAXV - 1);
	assert_string_equal(o.ev, "active? unhook0 release0 ref2 hook2 caption9");
	si_clear(&o);
	si_set_video_encoder2(&o, &V2, 1);
	assert_string_equal(o.ev, "active? unhook0 release0 ref2 hook2 caption1");
	si_clear(&o);
	si_set_video_encoder2(&o, &V2, MAXV);
	assert_string_equal(o.ev, "active?");
	si_clear(&o);
	si_set_video_encoder2(&o, &V2, SIZE_MAX);
	assert_string_equal(o.ev, "active?");
	/* OBS_OUTPUT_MULTI_TRACK alone is audio: video stays single track */
	o = si_new(F_ENCODED | F_VIDEO | MT_AUDIO);
	si_set_video_encoder2(&o, &V2, 1);
	assert_string_equal(o.ev, "active?");
	assert_null(o.video[1]);
}

static void e2e_video_replace(void **state)
{
	(void)state;
	struct stand_in o = si_new(F_ENCODED | F_VIDEO | MT_VIDEO);
	si_set_video_encoder2(&o, &V1, 0);
	/* the same encoder twice: nothing after the active load */
	si_clear(&o);
	si_set_video_encoder2(&o, &V1, 0);
	assert_string_equal(o.ev, "active?");
	assert_int_equal(o.captions[0], 1);
	/* another one: the old is unhooked and released first, the caption
	 * track replaced */
	si_clear(&o);
	si_set_video_encoder2(&o, &V2, 0);
	assert_string_equal(o.ev, "active? unhook1 release1 ref2 hook2 uncaption0 caption0");
	assert_int_equal(o.captions[0], 2);
	/* NULL clears the slot and destroys its caption track */
	si_clear(&o);
	si_set_video_encoder2(&o, NULL, 0);
	assert_string_equal(o.ev, "active? unhook2 release2 ref0 hook0 uncaption0");
	assert_null(o.video[0]);
	assert_int_equal(o.captions[0], 0);
	/* NULL on an empty slot: the same encoder, nothing */
	si_clear(&o);
	si_set_video_encoder2(&o, NULL, 3);
	assert_string_equal(o.ev, "active?");
	/* an encoder whose reference fails leaves the slot empty but still gets
	 * a caption track (characterized, not endorsed) */
	si_clear(&o);
	si_set_video_encoder2(&o, &D6, 4);
	assert_string_equal(o.ev, "active? unhook0 release0 ref6 hook0 caption4");
	assert_null(o.video[4]);
	assert_int_not_equal(o.captions[4], 0);
}

static void e2e_video_preferred_size(void **state)
{
	(void)state;
	/* index 0 with both axes set: the encoder takes the scaled size */
	struct stand_in o = si_new(F_ENCODED | F_VIDEO | MT_VIDEO);
	o.scaled_width = 1280;
	o.scaled_height = 720;
	si_set_video_encoder2(&o, &V1, 0);
	assert_string_equal(o.ev, "active? unhook0 release0 ref1 hook1 caption0 size1=1280x720");
	/* any other index: not */
	si_clear(&o);
	si_set_video_encoder2(&o, &V2, 1);
	assert_string_equal(o.ev, "active? unhook0 release0 ref2 hook2 caption1");
	/* one axis 0: not */
	o = si_new(F_ENCODED | F_VIDEO);
	o.scaled_width = 1280;
	si_set_video_encoder2(&o, &V1, 0);
	assert_string_equal(o.ev, "active? unhook0 release0 ref1 hook1 caption0");
	o = si_new(F_ENCODED | F_VIDEO);
	o.scaled_height = 720;
	si_set_video_encoder2(&o, &V1, 0);
	assert_string_equal(o.ev, "active? unhook0 release0 ref1 hook1 caption0");
	/* clearing index 0 with a scaled size: the size is still set, on the
	 * NULL slot, so obs_encoder_set_scaled_size() logs a NULL parameter
	 * (characterized, not endorsed) */
	o = si_new(F_ENCODED | F_VIDEO);
	o.scaled_width = 1280;
	o.scaled_height = 720;
	o.video[0] = &V1;
	si_set_video_encoder2(&o, NULL, 0);
	assert_string_equal(o.ev, "active? unhook1 release1 ref0 hook0 size0=1280x720");
}

static void e2e_audio(void **state)
{
	(void)state;
	/* a raw output, then a video-only encoded output */
	struct stand_in o = si_new(F_AUDIO);
	si_set_audio_encoder(&o, &A3, 0);
	assert_string_equal(o.ev, "warn-flag");
	o = si_new(F_ENCODED | F_VIDEO);
	si_set_audio_encoder(&o, &A3, 0);
	assert_string_equal(o.ev, "warn-flag");
	/* the wrong type, then active */
	o = si_new(F_ENCODED | F_AUDIO);
	o.active = true;
	si_set_audio_encoder(&o, &V1, 0);
	assert_string_equal(o.ev, "warn-type");
	si_clear(&o);
	si_set_audio_encoder(&o, &A3, 0);
	assert_string_equal(o.ev, "active? warn-active");
	/* single track: 0 only, no caption track, no size */
	o = si_new(F_ENCODED | F_AUDIO);
	o.scaled_width = 1280;
	o.scaled_height = 720;
	si_set_audio_encoder(&o, &A3, 1);
	assert_string_equal(o.ev, "active?");
	si_clear(&o);
	si_set_audio_encoder(&o, &A3, 0);
	assert_string_equal(o.ev, "active? unhook0 release0 ref3 hook3");
	si_clear(&o);
	si_set_audio_encoder(&o, &A3, 0);
	assert_string_equal(o.ev, "active?");
	si_clear(&o);
	si_set_audio_encoder(&o, &A4, 0);
	assert_string_equal(o.ev, "active? unhook3 release3 ref4 hook4");
	si_clear(&o);
	si_set_audio_encoder(&o, NULL, 0);
	assert_string_equal(o.ev, "active? unhook4 release4 ref0 hook0");
	/* multitrack audio (OBS_OUTPUT_MULTI_TRACK): below 6 */
	o = si_new(F_ENCODED | F_AUDIO | MT_AUDIO);
	si_set_audio_encoder(&o, &A4, MAXA - 1);
	assert_string_equal(o.ev, "active? unhook0 release0 ref4 hook4");
	static const size_t refused[] = {MAXA, MAXV - 1, SIZE_MAX};
	for (size_t i = 0; i < 3; i++) {
		si_clear(&o);
		si_set_audio_encoder(&o, &A3, refused[i]);
		assert_string_equal(o.ev, "active?");
	}
	/* OBS_OUTPUT_MULTI_TRACK_VIDEO says nothing about audio */
	o = si_new(F_ENCODED | F_AUDIO | MT_VIDEO);
	si_set_audio_encoder(&o, &A3, 1);
	assert_string_equal(o.ev, "active?");
}

static void e2e_getters(void **state)
{
	(void)state;
	/* the bounds only: a single-track output still answers slot 9 */
	struct stand_in o = si_new(F_ENCODED | F_VIDEO | F_AUDIO);
	o.video[3] = &V2;
	o.video[9] = &V1;
	o.audio[2] = &A4;
	o.audio[5] = &A3;
	for (size_t i = 0; i < N_EDGE_IDX; i++) {
		size_t idx = edge_idx[i];
		assert_int_equal(si_get_video_encoder2(&o, idx), idx < MAXV ? si_id(o.video[idx]) : -1);
		assert_int_equal(si_get_audio_encoder(&o, idx), idx < MAXA ? si_id(o.audio[idx]) : -1);
	}
	assert_int_equal(si_get_video_encoder2(&o, MAXV - 1), 1);
	assert_int_equal(si_get_audio_encoder(&o, MAXA - 1), 3);
	/* obs_output_get_video_encoder(): the first occupied slot */
	assert_int_equal(si_get_video_encoder(&o), 2);
	o.video[3] = NULL;
	assert_int_equal(si_get_video_encoder(&o), 1);
	o.video[9] = NULL;
	assert_int_equal(si_get_video_encoder(&o), -1);
	/* obs_output_audio(): the first audio encoder's, or the output's own */
	assert_int_equal(si_audio(&o), 4);
	o.audio[2] = NULL;
	assert_int_equal(si_audio(&o), 3);
	o.audio[5] = NULL;
	assert_int_equal(si_audio(&o), -1);
	o.flags = F_AUDIO;
	o.audio[0] = &A3;
	assert_int_equal(si_audio(&o), 100);
}

static void e2e_mixer(void **state)
{
	(void)state;
	struct stand_in o = si_new(F_AUDIO);
	o.mixer_mask = 7;
	si_set_mixer(&o, MIXES - 1);
	assert_string_equal(o.ev, "active?");
	assert_int_equal(o.mixer_mask, 1 << 5);
	si_clear(&o);
	si_set_mixer(&o, MIXES);
	assert_string_equal(o.ev, "warn-mixer");
	assert_int_equal(o.mixer_mask, 1 << 5);
	si_clear(&o);
	si_set_mixer(&o, SIZE_MAX);
	assert_string_equal(o.ev, "warn-mixer");
	si_clear(&o);
	si_set_mixer(&o, 0);
	assert_int_equal(o.mixer_mask, 1);
	/* active: the range warning first, then the active refusal */
	o.active = true;
	si_clear(&o);
	si_set_mixer(&o, MIXES);
	assert_string_equal(o.ev, "warn-mixer");
	si_clear(&o);
	si_set_mixer(&o, 3);
	assert_string_equal(o.ev, "active?");
	assert_int_equal(o.mixer_mask, 1);
	/* an encoded output: the raw warning */
	o = si_new(F_ENCODED | F_AUDIO);
	si_set_mixer(&o, 1);
	assert_string_equal(o.ev, "warn-flag");
	assert_int_equal(o.mixer_mask, 0);
}

static void e2e_remove(void **state)
{
	(void)state;
	struct stand_in o = si_new(F_ENCODED | F_VIDEO | F_AUDIO | MT_VIDEO | MT_AUDIO);
	o.video[0] = &V1;
	o.video[4] = &V1;
	o.video[9] = &V2;
	o.audio[1] = &A3;
	o.audio[5] = &A3;
	/* every slot of the encoder's kind that holds it, in order */
	si_remove_encoder_internal(&o, &V1);
	assert_string_equal(o.ev, "release1 release1");
	assert_null(o.video[0]);
	assert_null(o.video[4]);
	assert_ptr_equal(o.video[9], &V2);
	si_clear(&o);
	si_remove_encoder_internal(&o, &A3);
	assert_string_equal(o.ev, "release3 release3");
	assert_null(o.audio[1]);
	assert_null(o.audio[5]);
	/* an encoder in no slot, and one of neither type: nothing */
	si_clear(&o);
	si_remove_encoder_internal(&o, &A4);
	si_remove_encoder_internal(&o, &X5);
	assert_string_equal(o.ev, "");
	/* a video encoder in an audio slot is not looked for there
	 * (characterized; the setters refuse to put it there) */
	o.audio[0] = &V2;
	si_remove_encoder_internal(&o, &V2);
	assert_null(o.video[9]);
	assert_ptr_equal(o.audio[0], &V2);
}

static void e2e_preferred_size(void **state)
{
	(void)state;
	/* a raw video output: index 0 sets the scaled size, no encoder */
	struct stand_in o = si_new(F_VIDEO);
	si_set_preferred_size2(&o, 1280, 720, 0);
	assert_string_equal(o.ev, "active?");
	assert_int_equal(o.scaled_width, 1280);
	assert_int_equal(o.scaled_height, 720);
	/* other indexes below 10 change nothing on a raw output */
	si_clear(&o);
	si_set_preferred_size2(&o, 640, 360, 1);
	assert_string_equal(o.ev, "active?");
	assert_int_equal(o.scaled_width, 1280);
	si_clear(&o);
	si_set_preferred_size2(&o, 640, 360, MAXV);
	si_set_preferred_size2(&o, 640, 360, SIZE_MAX);
	assert_string_equal(o.ev, "");
	/* an audio-only output: the video warning */
	o = si_new(F_AUDIO);
	si_set_preferred_size2(&o, 640, 360, 0);
	assert_string_equal(o.ev, "warn-flag");
	/* active: the warning, nothing set */
	o = si_new(F_VIDEO);
	o.active = true;
	si_set_preferred_size2(&o, 640, 360, 0);
	assert_string_equal(o.ev, "active? warn-active");
	assert_int_equal(o.scaled_width, 0);
	/* encoded: index 0 sets both; the encoder at the index takes the size */
	o = si_new(F_ENCODED | F_VIDEO);
	o.video[0] = &V1;
	o.video[MAXV - 1] = &V2;
	si_set_preferred_size2(&o, 1920, 1080, 0);
	assert_string_equal(o.ev, "active? size1=1920x1080");
	assert_int_equal(o.scaled_width, 1920);
	si_clear(&o);
	si_set_preferred_size2(&o, 640, 360, MAXV - 1);
	assert_string_equal(o.ev, "active? size2=640x360");
	assert_int_equal(o.scaled_width, 1920);
	si_clear(&o);
	si_set_preferred_size2(&o, 640, 360, 1);
	assert_string_equal(o.ev, "active?");
	/* a 0 size is passed on as it is */
	si_clear(&o);
	si_set_preferred_size2(&o, 0, 0, 0);
	assert_string_equal(o.ev, "active? size1=0x0");
	assert_int_equal(o.scaled_width, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(first_video),
		cmocka_unit_test(first_audio),
		cmocka_unit_test(mixer),
		cmocka_unit_test(type_gates),
		cmocka_unit_test(index_rules),
		cmocka_unit_test(default_and_bounds),
		cmocka_unit_test(e2e_video_flags),
		cmocka_unit_test(e2e_video_index),
		cmocka_unit_test(e2e_video_replace),
		cmocka_unit_test(e2e_video_preferred_size),
		cmocka_unit_test(e2e_audio),
		cmocka_unit_test(e2e_getters),
		cmocka_unit_test(e2e_mixer),
		cmocka_unit_test(e2e_remove),
		cmocka_unit_test(e2e_preferred_size),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}

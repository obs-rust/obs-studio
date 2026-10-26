#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-encoder-bind.h"
#include "obs-encoder-audio-start.h"

/*
 * Characterization of the encoder media binding and shutdown checks of
 * libobs/obs-encoder.c, moved to libobs/obs-encoder-bind.c before their Rust
 * port:
 * - the audio type refusal of obs_encoder_set_audio() and obs_encoder_audio()
 *   (obs_encoder_bind_not_audio); the video setter and getters and
 *   obs_encoder_unbind_video() ask obs_encoder_audio_start_not_video(), the
 *   refusal obs_encoder_set_gpu_scale_type() already asks;
 * - the timebase encoder_set_video() and obs_encoder_set_audio() store
 *   (_video_timebase, _audio_timebase) and whether encoder_set_video()
 *   creates the frame rate divisor video (_divisor_override);
 * - obs_encoder_shutdown_unstarted(): each trylock result (_locked) and the
 *   shutdown itself (_shutdown_unstarted).
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The bodies of obs_encoder_set_video(), encoder_set_video(),
 * obs_encoder_set_audio(), the three media getters,
 * obs_encoder_shutdown_unstarted(), obs_encoder_unbind_video() and
 * obs_encoder_rebind_video() are also copied below onto a stand-in encoder
 * (no lock, no video output; the trylock results and the outputs' states are
 * given), so every gate is pinned end to end for audio and video encoders,
 * active and idle, initialized and not.
 *
 * The functions are hidden in libobs, so this test compiles the C sources
 * into itself and calls only them, through their headers. The real-libobs
 * tests test_encoder_video_reset (the unbind and rebind across a video
 * reset), test_output_failed_start_encoders (the unstarted shutdown),
 * test_encoding_lag_rescale, test_encoder_tex_active_null,
 * test_multitrack_encoder_override and test_texture_encoder_raw_format cover
 * the callers.
 *
 * Nothing here comes from outside OBS: the frame rate and sample rate come
 * from the video and audio settings, the divisor from the frontend.
 */

#define TYPE_AUDIO 0
#define TYPE_VIDEO 1

static const int edge_type[] = {INT_MIN, -1, TYPE_AUDIO, TYPE_VIDEO, 2, 3, INT_MAX};
#define N_EDGE_TYPE (sizeof(edge_type) / sizeof(edge_type[0]))
static const uint32_t edge_u32[] = {0, 1, 2, 60, 1001, 30000, 44100, 48000, 0x7fffffffU, 0x80000000U, UINT32_MAX};
#define N_EDGE_U32 (sizeof(edge_u32) / sizeof(edge_u32[0]))

/* ------------------------------------------------------------------------ */
/* the constant and the type refusals */

static void constants(void **state)
{
	(void)state;
	/* OBS_ENCODER_AUDIO; the video refusal's OBS_ENCODER_VIDEO */
	assert_int_equal(OBS_ENCODER_BIND_TYPE_AUDIO, 0);
	assert_int_equal(OBS_ENCODER_AUDIO_START_TYPE_VIDEO, 1);
}

static void type_refusals(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_TYPE; i++) {
		const int t = edge_type[i];
		assert_int_equal(obs_encoder_bind_not_audio(t), t != TYPE_AUDIO);
		assert_int_equal(obs_encoder_audio_start_not_video(t), t != TYPE_VIDEO);
	}
	/* an unknown type is refused by both */
	assert_true(obs_encoder_bind_not_audio(2));
	assert_true(obs_encoder_audio_start_not_video(2));
	assert_true(obs_encoder_bind_not_audio(-1));
	assert_true(obs_encoder_audio_start_not_video(-1));
}

/* ------------------------------------------------------------------------ */
/* the timebases */

static void video_timebase(void **state)
{
	(void)state;
	struct obs_encoder_bind_timebase tb;

	/* the frame period: fps_den over fps_num */
	tb = obs_encoder_bind_video_timebase(true, 30000, 1001);
	assert_int_equal(tb.num, 1001);
	assert_int_equal(tb.den, 30000);
	tb = obs_encoder_bind_video_timebase(true, 60, 1);
	assert_int_equal(tb.num, 1);
	assert_int_equal(tb.den, 60);

	/* fps_num 0: a timebase den of 0, stored as is (the fps-0 class: pinned,
	 * held; the frame rate comes from the video settings, which refuse it) */
	tb = obs_encoder_bind_video_timebase(true, 0, 1);
	assert_int_equal(tb.num, 1);
	assert_int_equal(tb.den, 0);
	tb = obs_encoder_bind_video_timebase(true, 0, 0);
	assert_int_equal(tb.num, 0);
	assert_int_equal(tb.den, 0);

	for (size_t i = 0; i < N_EDGE_U32; i++) {
		for (size_t j = 0; j < N_EDGE_U32; j++) {
			tb = obs_encoder_bind_video_timebase(true, edge_u32[i], edge_u32[j]);
			assert_int_equal(tb.num, edge_u32[j]);
			assert_int_equal(tb.den, edge_u32[i]);

			/* no video: 0 over 0, whatever is passed */
			tb = obs_encoder_bind_video_timebase(false, edge_u32[i], edge_u32[j]);
			assert_int_equal(tb.num, 0);
			assert_int_equal(tb.den, 0);
		}
	}
}

static void audio_timebase(void **state)
{
	(void)state;
	struct obs_encoder_bind_timebase tb;

	tb = obs_encoder_bind_audio_timebase(true, 48000);
	assert_int_equal(tb.num, 1);
	assert_int_equal(tb.den, 48000);
	tb = obs_encoder_bind_audio_timebase(true, 44100);
	assert_int_equal(tb.num, 1);
	assert_int_equal(tb.den, 44100);

	/* a rate of 0: 1 over 0, stored as is (characterized, not endorsed;
	 * the audio settings never give 0) */
	tb = obs_encoder_bind_audio_timebase(true, 0);
	assert_int_equal(tb.num, 1);
	assert_int_equal(tb.den, 0);

	for (size_t i = 0; i < N_EDGE_U32; i++) {
		tb = obs_encoder_bind_audio_timebase(true, edge_u32[i]);
		assert_int_equal(tb.num, 1);
		assert_int_equal(tb.den, edge_u32[i]);

		tb = obs_encoder_bind_audio_timebase(false, edge_u32[i]);
		assert_int_equal(tb.num, 0);
		assert_int_equal(tb.den, 0);
	}
}

static void divisor_override(void **state)
{
	(void)state;
	assert_false(obs_encoder_bind_divisor_override(0));
	assert_true(obs_encoder_bind_divisor_override(1));
	assert_true(obs_encoder_bind_divisor_override(2));
	for (size_t i = 0; i < N_EDGE_U32; i++)
		assert_int_equal(obs_encoder_bind_divisor_override(edge_u32[i]), edge_u32[i] != 0);
}

/* ------------------------------------------------------------------------ */
/* the unstarted shutdown */

static void locked(void **state)
{
	(void)state;
	static const int results[] = {INT_MIN, -1, 1, 11 /* EAGAIN */, 16 /* EBUSY */, 22 /* EINVAL */, INT_MAX};

	assert_true(obs_encoder_bind_locked(0));
	for (size_t i = 0; i < sizeof(results) / sizeof(results[0]); i++)
		assert_false(obs_encoder_bind_locked(results[i]));
}

static void shutdown_unstarted(void **state)
{
	(void)state;
	assert_true(obs_encoder_bind_shutdown_unstarted(true, false));
	assert_false(obs_encoder_bind_shutdown_unstarted(true, true));
	assert_false(obs_encoder_bind_shutdown_unstarted(false, false));
	assert_false(obs_encoder_bind_shutdown_unstarted(false, true));
}

/* ------------------------------------------------------------------------ */
/* the callers, copied onto a stand-in encoder */

struct stand_video {
	uint32_t fps_num;
	uint32_t fps_den;
};

struct stand_audio {
	uint32_t rate;
};

/* the frame rate divisor video encoder_set_video() creates */
static const struct stand_video override_video = {0, 0};

struct stand_encoder {
	int type;
	bool active;
	bool initialized;
	const void *media;
	const void *original_video;
	const struct stand_video *fps_override;
	uint32_t frame_rate_divisor;
	uint32_t timebase_num;
	uint32_t timebase_den;
	/* the outputs' obs_output_needs_encoders() */
	const bool *needs;
	size_t num_outputs;
	/* counted side effects */
	int warnings;
	int shutdowns;
	int overrides_created;
	int overrides_freed;
	size_t needs_asked;
	int unlocks;
};

static void stand_init(struct stand_encoder *e, int type, bool active, bool initialized)
{
	*e = (struct stand_encoder){0};
	e->type = type;
	e->active = active;
	e->initialized = initialized;
	e->timebase_num = 7;
	e->timebase_den = 7;
}

/* encoder_set_video() */
static void stand_encoder_set_video(struct stand_encoder *e, const struct stand_video *video)
{
	struct obs_encoder_bind_timebase timebase;

	if (e->fps_override) {
		e->overrides_freed++;
		e->fps_override = NULL;
	}

	if (video) {
		e->media = video;
		timebase = obs_encoder_bind_video_timebase(true, video->fps_num, video->fps_den);
		e->timebase_num = timebase.num;
		e->timebase_den = timebase.den;

		if (obs_encoder_bind_divisor_override(e->frame_rate_divisor)) {
			e->overrides_created++;
			e->fps_override = &override_video;
		}
	} else {
		e->media = NULL;
		timebase = obs_encoder_bind_video_timebase(false, 0, 0);
		e->timebase_num = timebase.num;
		e->timebase_den = timebase.den;
	}
}

/* obs_encoder_set_video() after obs_encoder_valid() */
static void stand_set_video(struct stand_encoder *e, const struct stand_video *video)
{
	if (obs_encoder_audio_start_not_video(e->type)) {
		e->warnings++;
		return;
	}
	if (e->active) {
		e->warnings++;
		return;
	}
	if (e->initialized) {
		e->warnings++;
		return;
	}
	stand_encoder_set_video(e, video);
}

/* obs_encoder_set_audio() after obs_encoder_valid() */
static void stand_set_audio(struct stand_encoder *e, const struct stand_audio *audio)
{
	if (obs_encoder_bind_not_audio(e->type)) {
		e->warnings++;
		return;
	}
	if (e->active) {
		e->warnings++;
		return;
	}

	struct obs_encoder_bind_timebase timebase;

	if (audio) {
		e->media = audio;
		timebase = obs_encoder_bind_audio_timebase(true, audio->rate);
		e->timebase_num = timebase.num;
		e->timebase_den = timebase.den;
	} else {
		e->media = NULL;
		timebase = obs_encoder_bind_audio_timebase(false, 0);
		e->timebase_num = timebase.num;
		e->timebase_den = timebase.den;
	}
}

/* obs_encoder_video(), obs_encoder_parent_video() and obs_encoder_audio()
 * after obs_encoder_valid() */
static const void *stand_video(struct stand_encoder *e)
{
	if (obs_encoder_audio_start_not_video(e->type)) {
		e->warnings++;
		return NULL;
	}
	return e->fps_override ? (const void *)e->fps_override : e->media;
}

static const void *stand_parent_video(struct stand_encoder *e)
{
	if (obs_encoder_audio_start_not_video(e->type)) {
		e->warnings++;
		return NULL;
	}
	return e->media;
}

static const void *stand_audio(struct stand_encoder *e)
{
	if (obs_encoder_bind_not_audio(e->type)) {
		e->warnings++;
		return NULL;
	}
	return e->media;
}

/* obs_encoder_shutdown_unstarted(), with the two trylock results given */
static void stand_shutdown_unstarted(struct stand_encoder *e, int outputs_trylock, int init_trylock)
{
	if (!obs_encoder_bind_locked(outputs_trylock))
		return;
	if (!obs_encoder_bind_locked(init_trylock)) {
		e->unlocks++;
		return;
	}

	bool needed = e->active;
	for (size_t i = 0; !needed && i < e->num_outputs; i++) {
		e->needs_asked++;
		needed = e->needs[i];
	}

	if (obs_encoder_bind_shutdown_unstarted(e->initialized, needed)) {
		e->shutdowns++;
		e->initialized = false;
	}

	e->unlocks += 2;
}

/* obs_encoder_unbind_video() */
static const void *stand_unbind_video(struct stand_encoder *e, const struct stand_video *const *videos, size_t num)
{
	const void *video = NULL;
	bool bound = false;

	if (obs_encoder_audio_start_not_video(e->type))
		return NULL;

	for (size_t i = 0; e->media && i < num && !bound; i++)
		bound = e->media == videos[i];

	if (bound && !e->active) {
		if (e->initialized) {
			e->shutdowns++;
			e->initialized = false;
		}
		video = e->media;
		stand_encoder_set_video(e, NULL);
		e->original_video = NULL;
	}
	return video;
}

/* obs_encoder_rebind_video() */
static void stand_rebind_video(struct stand_encoder *e, const struct stand_video *video)
{
	if (!e->media && !e->initialized && !e->active)
		stand_encoder_set_video(e, video);
}

static const struct stand_video v_ntsc = {30000, 1001};
static const struct stand_video v_60 = {60, 1};
static const struct stand_video v_fps0 = {0, 1};
static const struct stand_audio a_48k = {48000};

static void video_setter_gates(void **state)
{
	(void)state;
	struct stand_encoder e;

	for (int t = 0; t < 2; t++) {
		const int type = t ? TYPE_VIDEO : TYPE_AUDIO;
		for (int active = 0; active < 2; active++) {
			for (int init = 0; init < 2; init++) {
				stand_init(&e, type, active, init);
				stand_set_video(&e, &v_60);
				const bool binds = type == TYPE_VIDEO && !active && !init;
				assert_int_equal(e.media == &v_60, binds);
				assert_int_equal(e.warnings, !binds);
				assert_int_equal(e.timebase_num, binds ? 1 : 7);
				assert_int_equal(e.timebase_den, binds ? 60 : 7);
			}
		}
	}

	/* an unknown type is refused before the active flag */
	stand_init(&e, 2, false, false);
	stand_set_video(&e, &v_60);
	assert_null(e.media);
	assert_int_equal(e.warnings, 1);

	/* NULL unbinds: no media, a timebase of 0 over 0 */
	stand_init(&e, TYPE_VIDEO, false, false);
	stand_set_video(&e, &v_ntsc);
	assert_ptr_equal(e.media, &v_ntsc);
	assert_int_equal(e.timebase_num, 1001);
	assert_int_equal(e.timebase_den, 30000);
	stand_set_video(&e, NULL);
	assert_null(e.media);
	assert_int_equal(e.timebase_num, 0);
	assert_int_equal(e.timebase_den, 0);

	/* fps_num 0: den 0 (pinned, held) */
	stand_set_video(&e, &v_fps0);
	assert_ptr_equal(e.media, &v_fps0);
	assert_int_equal(e.timebase_num, 1);
	assert_int_equal(e.timebase_den, 0);
}

static void audio_setter_gates(void **state)
{
	(void)state;
	struct stand_encoder e;

	for (int t = 0; t < 2; t++) {
		const int type = t ? TYPE_VIDEO : TYPE_AUDIO;
		for (int active = 0; active < 2; active++) {
			for (int init = 0; init < 2; init++) {
				stand_init(&e, type, active, init);
				stand_set_audio(&e, &a_48k);
				/* characterized, not endorsed: the audio setter has no
				 * initialized refusal, so an initialized idle audio
				 * encoder takes the new audio (the video setter refuses
				 * it) */
				const bool binds = type == TYPE_AUDIO && !active;
				assert_int_equal(e.media == &a_48k, binds);
				assert_int_equal(e.warnings, !binds);
				assert_int_equal(e.timebase_num, binds ? 1 : 7);
				assert_int_equal(e.timebase_den, binds ? 48000 : 7);
			}
		}
	}

	stand_init(&e, TYPE_AUDIO, false, true);
	stand_set_audio(&e, &a_48k);
	assert_ptr_equal(e.media, &a_48k);
	assert_true(e.initialized);
	stand_set_audio(&e, NULL);
	assert_null(e.media);
	assert_int_equal(e.timebase_num, 0);
	assert_int_equal(e.timebase_den, 0);

	stand_init(&e, -1, false, false);
	stand_set_audio(&e, &a_48k);
	assert_null(e.media);
	assert_int_equal(e.warnings, 1);
}

static void divisor_video(void **state)
{
	(void)state;
	struct stand_encoder e;

	/* divisor 0: no frame rate divisor video; the getter gives the media */
	stand_init(&e, TYPE_VIDEO, false, false);
	stand_set_video(&e, &v_60);
	assert_null(e.fps_override);
	assert_int_equal(e.overrides_created, 0);
	assert_ptr_equal(stand_video(&e), &v_60);
	assert_ptr_equal(stand_parent_video(&e), &v_60);

	/* divisor 1: one is created (the same rate), and the getter gives it */
	stand_init(&e, TYPE_VIDEO, false, false);
	e.frame_rate_divisor = 1;
	stand_set_video(&e, &v_60);
	assert_ptr_equal(e.fps_override, &override_video);
	assert_int_equal(e.overrides_created, 1);
	assert_ptr_equal(stand_video(&e), &override_video);
	assert_ptr_equal(stand_parent_video(&e), &v_60);

	/* binding again frees the old one first; unbinding frees it */
	stand_set_video(&e, &v_ntsc);
	assert_int_equal(e.overrides_freed, 1);
	assert_int_equal(e.overrides_created, 2);
	stand_set_video(&e, NULL);
	assert_int_equal(e.overrides_freed, 2);
	assert_int_equal(e.overrides_created, 2);
	assert_null(e.fps_override);
	assert_null(stand_video(&e));
}

static void getters(void **state)
{
	(void)state;
	struct stand_encoder e;

	stand_init(&e, TYPE_AUDIO, false, false);
	e.media = &a_48k;
	assert_ptr_equal(stand_audio(&e), &a_48k);
	assert_null(stand_video(&e));
	assert_null(stand_parent_video(&e));
	assert_int_equal(e.warnings, 2);

	stand_init(&e, TYPE_VIDEO, true, true);
	e.media = &v_60;
	/* the getters have no active or initialized gate */
	assert_ptr_equal(stand_video(&e), &v_60);
	assert_ptr_equal(stand_parent_video(&e), &v_60);
	assert_null(stand_audio(&e));
	assert_int_equal(e.warnings, 1);

	stand_init(&e, 2, false, false);
	e.media = &v_60;
	assert_null(stand_video(&e));
	assert_null(stand_parent_video(&e));
	assert_null(stand_audio(&e));
	assert_int_equal(e.warnings, 3);
}

static void unstarted_shutdown(void **state)
{
	(void)state;
	struct stand_encoder e;
	static const bool none[] = {false, false, false};
	static const bool second[] = {false, true, false};

	/* initialized, idle, no output needs it: shut down */
	for (int type = 0; type < 2; type++) {
		stand_init(&e, type, false, true);
		e.needs = none;
		e.num_outputs = 3;
		stand_shutdown_unstarted(&e, 0, 0);
		assert_int_equal(e.shutdowns, 1);
		assert_false(e.initialized);
		assert_int_equal(e.needs_asked, 3);
		assert_int_equal(e.unlocks, 2);
	}

	/* either mutex busy: left alone; outputs_mutex released when init_mutex
	 * is the busy one */
	stand_init(&e, TYPE_VIDEO, false, true);
	e.needs = none;
	e.num_outputs = 3;
	stand_shutdown_unstarted(&e, 16, 0);
	assert_int_equal(e.shutdowns, 0);
	assert_int_equal(e.unlocks, 0);
	stand_shutdown_unstarted(&e, 0, 16);
	assert_int_equal(e.shutdowns, 0);
	assert_int_equal(e.unlocks, 1);
	assert_true(e.initialized);
	assert_int_equal(e.needs_asked, 0);

	/* active: needed, and no output is asked */
	stand_init(&e, TYPE_VIDEO, true, true);
	e.needs = none;
	e.num_outputs = 3;
	stand_shutdown_unstarted(&e, 0, 0);
	assert_int_equal(e.shutdowns, 0);
	assert_int_equal(e.needs_asked, 0);
	assert_true(e.initialized);

	/* an output needs it: the walk stops there */
	stand_init(&e, TYPE_AUDIO, false, true);
	e.needs = second;
	e.num_outputs = 3;
	stand_shutdown_unstarted(&e, 0, 0);
	assert_int_equal(e.shutdowns, 0);
	assert_int_equal(e.needs_asked, 2);
	assert_true(e.initialized);

	/* not initialized: nothing to shut down, after the whole walk */
	stand_init(&e, TYPE_VIDEO, false, false);
	e.needs = none;
	e.num_outputs = 3;
	stand_shutdown_unstarted(&e, 0, 0);
	assert_int_equal(e.shutdowns, 0);
	assert_int_equal(e.needs_asked, 3);
	assert_int_equal(e.unlocks, 2);

	/* no outputs */
	stand_init(&e, TYPE_VIDEO, false, true);
	stand_shutdown_unstarted(&e, 0, 0);
	assert_int_equal(e.shutdowns, 1);
}

static void unbind_and_rebind(void **state)
{
	(void)state;
	struct stand_encoder e;
	const struct stand_video *videos[] = {&v_ntsc, &v_60};

	for (int bound = 0; bound < 2; bound++) {
		for (int active = 0; active < 2; active++) {
			for (int init = 0; init < 2; init++) {
				stand_init(&e, TYPE_VIDEO, false, false);
				e.frame_rate_divisor = 2;
				stand_set_video(&e, bound ? &v_60 : &v_fps0);
				e.original_video = &v_ntsc;
				e.active = active;
				e.initialized = init;

				const void *ret = stand_unbind_video(&e, videos, 2);
				const bool unbinds = bound && !active;
				assert_int_equal(ret != NULL, unbinds);
				if (unbinds) {
					assert_ptr_equal(ret, &v_60);
					assert_null(e.media);
					assert_null(e.fps_override);
					assert_null(e.original_video);
					assert_int_equal(e.timebase_num, 0);
					assert_int_equal(e.timebase_den, 0);
					assert_int_equal(e.shutdowns, init);
					assert_false(e.initialized);
				} else {
					assert_non_null(e.media);
					assert_ptr_equal(e.original_video, &v_ntsc);
					assert_int_equal(e.shutdowns, 0);
					assert_int_equal(e.initialized, init);
				}
			}
		}
	}

	/* an audio encoder is never unbound, whatever it holds */
	stand_init(&e, TYPE_AUDIO, false, true);
	e.media = &v_60;
	assert_null(stand_unbind_video(&e, videos, 2));
	assert_ptr_equal(e.media, &v_60);
	assert_true(e.initialized);

	/* no media, or no videos: nothing bound */
	stand_init(&e, TYPE_VIDEO, false, true);
	assert_null(stand_unbind_video(&e, videos, 2));
	assert_true(e.initialized);
	stand_init(&e, TYPE_VIDEO, false, true);
	e.media = &v_60;
	assert_null(stand_unbind_video(&e, videos, 0));
	assert_true(e.initialized);

	/* the rebind: only with no media, not initialized and not active */
	for (int media = 0; media < 2; media++) {
		for (int active = 0; active < 2; active++) {
			for (int init = 0; init < 2; init++) {
				stand_init(&e, TYPE_VIDEO, active, init);
				e.media = media ? &v_fps0 : NULL;
				stand_rebind_video(&e, &v_ntsc);
				const bool rebinds = !media && !init && !active;
				assert_int_equal(e.media == &v_ntsc, rebinds);
				assert_int_equal(e.timebase_den, rebinds ? 30000 : 7);
			}
		}
	}

	/* unbind, then rebind to the new video, keeps the divisor */
	stand_init(&e, TYPE_VIDEO, false, false);
	e.frame_rate_divisor = 2;
	stand_set_video(&e, &v_60);
	e.initialized = true;
	assert_ptr_equal(stand_unbind_video(&e, videos, 2), &v_60);
	stand_rebind_video(&e, &v_ntsc);
	assert_ptr_equal(e.media, &v_ntsc);
	assert_ptr_equal(e.fps_override, &override_video);
	assert_int_equal(e.timebase_num, 1001);
	assert_int_equal(e.timebase_den, 30000);
	assert_int_equal(e.shutdowns, 1);

	/* the rebind of a NULL video (no canvas) leaves it unbound */
	stand_init(&e, TYPE_VIDEO, false, false);
	stand_rebind_video(&e, NULL);
	assert_null(e.media);
	assert_int_equal(e.timebase_num, 0);
	assert_int_equal(e.timebase_den, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(type_refusals),
		cmocka_unit_test(video_timebase),
		cmocka_unit_test(audio_timebase),
		cmocka_unit_test(divisor_override),
		cmocka_unit_test(locked),
		cmocka_unit_test(shutdown_unstarted),
		cmocka_unit_test(video_setter_gates),
		cmocka_unit_test(audio_setter_gates),
		cmocka_unit_test(divisor_video),
		cmocka_unit_test(getters),
		cmocka_unit_test(unstarted_shutdown),
		cmocka_unit_test(unbind_and_rebind),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}

/* Original extracted raw video callback entry points over a real-header core,
 * the real obs_get_video() field adapter, and one shared recorder for
 * start_raw_video, stop_raw_video and blog. The unit owns no registration
 * algorithm; start/stop are exercised for real by the unchanged Tier 2 tests
 * against both libraries. No absent-core cases (C UB); a core with no main
 * canvas, no mix or a mix whose video is NULL is defined. */
#include "libobs/build/libobs.pch.h"
#include <stdarg.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#define RVC_TEXT 160

static struct obs_core rvc_core;
static struct obs_canvas rvc_canvas;
static struct obs_core_video_mix rvc_mixes[2];
static unsigned char rvc_tokens[4];
static struct video_scale_info rvc_conversions[2];
static size_t rvc_starts;
static size_t rvc_stops;
static size_t rvc_logs;
static size_t rvc_args[6];
static int rvc_level;
static char rvc_text[RVC_TEXT];

static void rvc_a(void *param, struct video_data *frame)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(frame);
}

static void rvc_b(void *param, struct video_data *frame)
{
	volatile unsigned marker = 1;
	(void)marker;
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(frame);
}

static void rvc_unknown(void *param, struct video_data *frame)
{
	volatile unsigned marker = 2;
	(void)marker;
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(frame);
}

typedef void (*rvc_callback_t)(void *param, struct video_data *frame);
static const rvc_callback_t rvc_callbacks[] = {NULL, rvc_a, rvc_b, rvc_unknown};

static void *rvc_param(size_t id)
{
	return id ? &rvc_tokens[id] : NULL;
}

static const struct video_scale_info *rvc_conversion(size_t id)
{
	return id ? &rvc_conversions[id - 1] : NULL;
}

static size_t rvc_video_id(const video_t *video)
{
	if (!video)
		return 0;
	if (video == (const video_t *)&rvc_tokens[0])
		return 1;
	if (video == (const video_t *)&rvc_tokens[3])
		return 2;
	return 3;
}

static size_t rvc_callback_id(rvc_callback_t callback)
{
	for (size_t i = 0; i < 4; ++i)
		if (callback == rvc_callbacks[i])
			return i;
	return 4;
}

static size_t rvc_param_id(const void *param)
{
	for (size_t i = 0; i < 4; ++i)
		if (param == rvc_param(i))
			return i;
	return 4;
}

static size_t rvc_conversion_id(const struct video_scale_info *conversion)
{
	for (size_t i = 0; i < 3; ++i)
		if (conversion == rvc_conversion(i))
			return i;
	return 3;
}

void oracle_raw_video_callbacks_start(video_t *v, const struct video_scale_info *conversion,
				      uint32_t frame_rate_divisor, rvc_callback_t callback, void *param);
void oracle_raw_video_callbacks_start(video_t *v, const struct video_scale_info *conversion,
				      uint32_t frame_rate_divisor, rvc_callback_t callback, void *param)
{
	rvc_starts++;
	rvc_args[0] = rvc_video_id(v);
	rvc_args[1] = rvc_conversion_id(conversion);
	rvc_args[2] = frame_rate_divisor;
	rvc_args[3] = rvc_callback_id(callback);
	rvc_args[4] = rvc_param_id(param);
}

void oracle_raw_video_callbacks_stop(video_t *v, rvc_callback_t callback, void *param);
void oracle_raw_video_callbacks_stop(video_t *v, rvc_callback_t callback, void *param)
{
	rvc_stops++;
	rvc_args[0] = rvc_video_id(v);
	rvc_args[3] = rvc_callback_id(callback);
	rvc_args[4] = rvc_param_id(param);
}

/* Keeps the level and the format text; the wrapper passes no arguments. */
void oracle_raw_video_callbacks_log(int level, const char *format, ...);
void oracle_raw_video_callbacks_log(int level, const char *format, ...)
{
	rvc_logs++;
	rvc_level = level;
	memset(rvc_text, 0, sizeof(rvc_text));
	if (strlen(format) >= sizeof(rvc_text))
		abort();
	memcpy(rvc_text, format, strlen(format));
}

/* The real field adapter of obs_get_video(), renamed and compiled against
 * this test core; obs-core's own definition is under the real name. The
 * rename also renames its struct tag. */
#define obs (&rvc_core)
#define obs_get_video_fields oracle_raw_video_callbacks_fields
#include "get-video-fields.inc"
#undef obs_get_video_fields

void oracle_obs_add_raw_video_callback(const struct video_scale_info *conversion, rvc_callback_t callback, void *param);
void oracle_obs_add_raw_video_callback2(const struct video_scale_info *conversion, uint32_t frame_rate_divisor,
					rvc_callback_t callback, void *param);
void oracle_obs_remove_raw_video_callback(rvc_callback_t callback, void *param);
#define obs_add_raw_video_callback oracle_obs_add_raw_video_callback
#define obs_add_raw_video_callback2 oracle_obs_add_raw_video_callback2
#define obs_remove_raw_video_callback oracle_obs_remove_raw_video_callback
#define start_raw_video oracle_raw_video_callbacks_start
#define stop_raw_video oracle_raw_video_callbacks_stop
#define blog oracle_raw_video_callbacks_log
#include "libobs/obs-raw-video-callbacks.c.h"
#undef blog
#undef stop_raw_video
#undef start_raw_video
#undef obs_remove_raw_video_callback
#undef obs_add_raw_video_callback2
#undef obs_add_raw_video_callback
#undef obs

typedef void (*rvc_add_t)(const struct video_scale_info *, rvc_callback_t, void *);
typedef void (*rvc_add2_t)(const struct video_scale_info *, uint32_t, rvc_callback_t, void *);
typedef void (*rvc_remove_t)(rvc_callback_t, void *);

/* state: 0 no main canvas, 1 a canvas with no mix, 2 a mix whose video is
 * NULL, 3 mix 0 with video 1, 4 mix 1 with video 2. op: 0 add, 1 add2,
 * 2 remove. */
void oracle_raw_video_callbacks_run(rvc_add_t add, rvc_add2_t add2, rvc_remove_t remove, size_t state, size_t op,
				    size_t conversion, uint32_t divisor, size_t callback, size_t param, size_t repeats,
				    size_t *report, char *text);
void oracle_raw_video_callbacks_run(rvc_add_t add, rvc_add2_t add2, rvc_remove_t remove, size_t state, size_t op,
				    size_t conversion, uint32_t divisor, size_t callback, size_t param, size_t repeats,
				    size_t *report, char *text)
{
	if (state > 4 || op > 2 || conversion >= 3 || callback >= 4 || param >= 4 || repeats == 0 || repeats > 3)
		abort();
	memset(&rvc_core, 0x5a, sizeof(rvc_core));
	memset(&rvc_canvas, 0x5a, sizeof(rvc_canvas));
	memset(rvc_mixes, 0x5a, sizeof(rvc_mixes));
	rvc_mixes[0].video = state == 2 ? NULL : (video_t *)&rvc_tokens[0];
	rvc_mixes[1].video = (video_t *)&rvc_tokens[3];
	rvc_canvas.mix = state <= 1 ? NULL : &rvc_mixes[state == 4 ? 1 : 0];
	rvc_core.data.main_canvas = state == 0 ? NULL : &rvc_canvas;
	rvc_conversions[0] = (struct video_scale_info){VIDEO_FORMAT_I420, 0, 0, VIDEO_RANGE_PARTIAL, VIDEO_CS_709};
	rvc_conversions[1] = (struct video_scale_info){VIDEO_FORMAT_NV12, 32, 16, VIDEO_RANGE_FULL, VIDEO_CS_SRGB};

	struct obs_core core;
	struct obs_canvas canvas;
	struct obs_core_video_mix mixes[2];
	struct video_scale_info conversions[2];
	memcpy(&core, &rvc_core, sizeof(core));
	memcpy(&canvas, &rvc_canvas, sizeof(canvas));
	memcpy(mixes, rvc_mixes, sizeof(mixes));
	memcpy(conversions, rvc_conversions, sizeof(conversions));
	rvc_starts = rvc_stops = rvc_logs = 0;
	rvc_level = 0;
	memset(rvc_args, 0, sizeof(rvc_args));
	memset(rvc_text, 0, sizeof(rvc_text));

	for (size_t i = 0; i < repeats; ++i) {
		if (op == 0)
			add(rvc_conversion(conversion), rvc_callbacks[callback], rvc_param(param));
		else if (op == 1)
			add2(rvc_conversion(conversion), divisor, rvc_callbacks[callback], rvc_param(param));
		else
			remove(rvc_callbacks[callback], rvc_param(param));
	}

	memset(report, 0, 12 * sizeof(*report));
	report[0] = rvc_starts;
	report[1] = rvc_stops;
	report[2] = rvc_logs;
	memcpy(report + 3, rvc_args, 5 * sizeof(*report));
	report[8] = (size_t)(unsigned)rvc_level;
	report[9] = memcmp(&core, &rvc_core, sizeof(core)) == 0 && memcmp(&canvas, &rvc_canvas, sizeof(canvas)) == 0 &&
		    memcmp(mixes, rvc_mixes, sizeof(mixes)) == 0;
	report[10] = memcmp(conversions, rvc_conversions, sizeof(conversions)) == 0;
	report[11] = (size_t)LOG_WARNING;
	memcpy(text, rvc_text, sizeof(rvc_text));
	memset(&rvc_core, 0, sizeof(rvc_core));
	memset(&rvc_canvas, 0, sizeof(rvc_canvas));
	memset(rvc_mixes, 0, sizeof(rvc_mixes));
}

void oracle_raw_video_callbacks_layout(size_t *out);
void oracle_raw_video_callbacks_layout(size_t *out)
{
	out[0] = sizeof(struct video_scale_info);
	out[1] = _Alignof(struct video_scale_info);
	out[2] = offsetof(struct video_scale_info, format);
	out[3] = offsetof(struct video_scale_info, width);
	out[4] = offsetof(struct video_scale_info, height);
	out[5] = offsetof(struct video_scale_info, range);
	out[6] = offsetof(struct video_scale_info, colorspace);
	out[7] = sizeof(rvc_callback_t);
	out[8] = _Alignof(rvc_callback_t);
	out[9] = sizeof(uint32_t);
	out[10] = sizeof(video_t *);
	out[11] = sizeof(struct oracle_raw_video_callbacks_fields);
	out[12] = offsetof(struct oracle_raw_video_callbacks_fields, canvas);
	out[13] = offsetof(struct oracle_raw_video_callbacks_fields, mix);
	out[14] = offsetof(struct oracle_raw_video_callbacks_fields, video);
}

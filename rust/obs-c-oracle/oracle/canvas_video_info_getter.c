/* Real-layout original getter, discovered adapters, and callback-only swap. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"

static _Thread_local struct obs_core *canvas_info_current;
#define obs canvas_info_current
bool oracle_obs_canvas_get_video_info(const obs_canvas_t *canvas, struct obs_video_info *ovi);
#define obs_canvas_get_video_info oracle_obs_canvas_get_video_info
#include "obs-canvas-video-info-getter.c.h"
#undef obs_canvas_get_video_info
#include "canvas_video_info_fields.inc"
#undef obs

typedef bool (*canvas_info_getter)(const obs_canvas_t *, struct obs_video_info *);
void oracle_canvas_video_info_layout(size_t *layout);
void oracle_canvas_video_info_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_video_info);
	layout[1] = _Alignof(struct obs_video_info);
#define CANVAS_INFO_OFFSET(n, field) layout[n] = offsetof(struct obs_video_info, field)
	CANVAS_INFO_OFFSET(2, graphics_module);
	CANVAS_INFO_OFFSET(3, fps_num);
	CANVAS_INFO_OFFSET(4, fps_den);
	CANVAS_INFO_OFFSET(5, base_width);
	CANVAS_INFO_OFFSET(6, base_height);
	CANVAS_INFO_OFFSET(7, output_width);
	CANVAS_INFO_OFFSET(8, output_height);
	CANVAS_INFO_OFFSET(9, output_format);
	CANVAS_INFO_OFFSET(10, adapter);
	CANVAS_INFO_OFFSET(11, gpu_conversion);
	CANVAS_INFO_OFFSET(12, colorspace);
	CANVAS_INFO_OFFSET(13, range);
	CANVAS_INFO_OFFSET(14, scale_type);
#undef CANVAS_INFO_OFFSET
}

bool oracle_canvas_video_info_check(canvas_info_getter candidate, bool graphics, bool mixed, bool null_canvas,
				  bool null_output, uint8_t value, uint32_t word);
bool oracle_canvas_video_info_check(canvas_info_getter candidate, bool graphics, bool mixed, bool null_canvas,
				  bool null_output, uint8_t value, uint32_t word)
{
	struct obs_core *core = calloc(1, sizeof(*core));
	struct obs_canvas canvas = {0}, before;
	struct obs_core core_before;
	struct obs_core_video_mix mix, mix_before;
	struct obs_video_info actual, expected;
	void *token = malloc(1);
	if (!core || !token) {
		free(core);
		free(token);
		return false;
	}
	*(uint8_t *)token = value;
	memset(&mix, 0, sizeof(mix));
	memset(&mix.ovi, value, sizeof(mix.ovi));
	mix.ovi.graphics_module = token;
	mix.ovi.gpu_conversion = value & 1;
	mix.ovi.fps_num = word;
	mix.ovi.output_height = ~word;
	core->video.graphics = graphics ? token : NULL;
	canvas.mix = mixed ? &mix : NULL;
	memcpy(&core_before, core, sizeof(*core));
	memcpy(&before, &canvas, sizeof(canvas));
	memcpy(&mix_before, &mix, sizeof(mix));
	memset(&actual, 0xa5, sizeof(actual));
	memset(&expected, 0xa5, sizeof(expected));
	canvas_info_current = core;
	const obs_canvas_t *input = null_canvas ? NULL : &canvas;
	bool c = oracle_obs_canvas_get_video_info(input, null_output ? NULL : &expected);
	bool rust = candidate(input, null_output ? NULL : &actual);
	bool alias = !c || (oracle_obs_canvas_get_video_info(input, &mix.ovi) && candidate(input, &mix.ovi));
	bool equal = alias && c == rust && c == (graphics && mixed) &&
		     memcmp(&actual, &expected, sizeof(actual)) == 0 &&
		     memcmp(&canvas, &before, sizeof(canvas)) == 0 &&
		     memcmp(&mix, &mix_before, sizeof(mix)) == 0 &&
		     memcmp(core, &core_before, sizeof(*core)) == 0 && *(uint8_t *)token == value;
	canvas_info_current = NULL;
	free(token);
	free(core);
	return equal;
}

/* Original extracted C, real headers and real adapters on thread-local state. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"

#if defined(_MSC_VER)
#define VIDEO_INFO_THREAD_LOCAL __declspec(thread)
#else
#define VIDEO_INFO_THREAD_LOCAL __thread
#endif
static VIDEO_INFO_THREAD_LOCAL struct obs_core *video_info_current;
#undef VIDEO_INFO_THREAD_LOCAL
#define obs video_info_current
bool oracle_video_info_query_get(struct obs_video_info *ovi);
#define obs_get_video_info oracle_video_info_query_get
#include "video_info_query.inc"
#undef obs_get_video_info
#include "video_info_query_fields.inc"
#undef obs

typedef bool (*video_info_getter)(struct obs_video_info *);
void oracle_video_info_query_layout(size_t *layout);
void oracle_video_info_query_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_video_info);
	layout[1] = _Alignof(struct obs_video_info);
#define VIDEO_INFO_OFFSET(n, field) layout[n] = offsetof(struct obs_video_info, field)
	VIDEO_INFO_OFFSET(2, graphics_module);
	VIDEO_INFO_OFFSET(3, fps_num);
	VIDEO_INFO_OFFSET(4, fps_den);
	VIDEO_INFO_OFFSET(5, base_width);
	VIDEO_INFO_OFFSET(6, base_height);
	VIDEO_INFO_OFFSET(7, output_width);
	VIDEO_INFO_OFFSET(8, output_height);
	VIDEO_INFO_OFFSET(9, output_format);
	VIDEO_INFO_OFFSET(10, adapter);
	VIDEO_INFO_OFFSET(11, gpu_conversion);
	VIDEO_INFO_OFFSET(12, colorspace);
	VIDEO_INFO_OFFSET(13, range);
	VIDEO_INFO_OFFSET(14, scale_type);
#undef VIDEO_INFO_OFFSET
}

bool oracle_video_info_query_check(video_info_getter candidate, bool present, bool graphics, bool mixed,
				   bool null_canvas, bool null_output, bool null_module, uint8_t value, uint32_t word);
bool oracle_video_info_query_check(video_info_getter candidate, bool present, bool graphics, bool mixed,
				   bool null_canvas, bool null_output, bool null_module, uint8_t value, uint32_t word)
{
	struct obs_core *core = calloc(1, sizeof(*core));
	struct obs_core *before = malloc(sizeof(*core));
	struct obs_canvas canvas = {0}, canvas_before;
	struct obs_core_video_mix mix, mix_before;
	struct obs_video_info actual, expected;
	char token = (char)value;
	if (!core || !before) {
		free(core);
		free(before);
		return false;
	}
	memset(&mix, 0, sizeof(mix));
	memset(&mix.ovi, value, sizeof(mix.ovi));
	mix.ovi.graphics_module = null_module ? NULL : &token;
	mix.ovi.gpu_conversion = value & 1;
	mix.ovi.fps_num = word;
	mix.ovi.fps_den = ~word;
	mix.ovi.output_height = word ^ 0x12345678;
	core->video.graphics = graphics ? (graphics_t *)&token : NULL;
	core->data.main_canvas = null_canvas ? NULL : &canvas;
	canvas.mix = mixed ? &mix : NULL;
	memcpy(before, core, sizeof(*core));
	memcpy(&canvas_before, &canvas, sizeof(canvas));
	memcpy(&mix_before, &mix, sizeof(mix));
	memset(&actual, 0xa5, sizeof(actual));
	memset(&expected, 0xa5, sizeof(expected));
	video_info_current = present ? core : NULL;
	bool c = oracle_video_info_query_get(null_output ? NULL : &expected);
	bool rust = candidate(null_output ? NULL : &actual);
	bool alias = !c || (oracle_video_info_query_get(&mix.ovi) && candidate(&mix.ovi));
	bool equal = alias && c == rust && c == (present && graphics && mixed) &&
		     memcmp(&actual, &expected, sizeof(actual)) == 0 &&
		     memcmp(&canvas, &canvas_before, sizeof(canvas)) == 0 &&
		     memcmp(&mix, &mix_before, sizeof(mix)) == 0 && memcmp(core, before, sizeof(*core)) == 0 &&
		     token == (char)value && video_info_current == (present ? core : NULL);
	video_info_current = NULL;
	free(before);
	free(core);
	return equal;
}

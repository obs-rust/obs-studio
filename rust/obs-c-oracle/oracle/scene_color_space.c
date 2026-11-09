/* Compile the original callback and real adapter, sharing a thread-local video query. */
#include "libobs/build/libobs.pch.h"

static THREAD_LOCAL bool scene_color_present;
static THREAD_LOCAL int scene_color_video;
static THREAD_LOCAL unsigned scene_color_queries;

static bool scene_color_video_info(struct obs_video_info *info)
{
	scene_color_queries++;
	/* Failure leaves the output untouched, as the real obs_get_video_info does. */
	if (!scene_color_present)
		return false;
	memset(info, 0, sizeof(*info));
	info->colorspace = (enum video_colorspace)scene_color_video;
	return true;
}

#define obs_get_video_info scene_color_video_info
#define scene_video_get_color_space oracle_scene_video_get_color_space
#include "libobs/obs-scene-color-space.c.h"
#undef scene_video_get_color_space
#include "scene-color-space-fields.inc"
#undef obs_get_video_info

typedef int (*scene_color_callback)(void *, size_t, const enum gs_color_space *);
unsigned oracle_scene_color_space_probe(bool present, int video, size_t count, const enum gs_color_space *preferences,
					scene_color_callback callback);
void oracle_scene_color_space_constants(int *values);

unsigned oracle_scene_color_space_probe(bool present, int video, size_t count, const enum gs_color_space *preferences,
					scene_color_callback callback)
{
	scene_color_present = present;
	scene_color_video = video;
	scene_color_queries = 0;
	unsigned char data = 0xa5;
	int expected = (int)oracle_scene_video_get_color_space(&data, count, preferences);
	unsigned result = scene_color_queries == 1 ? 0 : 1;
	scene_color_queries = 0;
	if (callback(&data, count, preferences) != expected)
		result |= 2;
	if (scene_color_queries != 1)
		result |= 4;
	if (data != 0xa5)
		result |= 8;
	return result;
}

void oracle_scene_color_space_constants(int *values)
{
	values[0] = GS_CS_SRGB;
	values[1] = GS_CS_709_EXTENDED;
	values[2] = VIDEO_CS_SRGB;
	values[3] = VIDEO_CS_2100_PQ;
	values[4] = VIDEO_CS_2100_HLG;
	values[5] = (int)sizeof(enum gs_color_space);
	values[6] = (int)sizeof(enum video_colorspace);
	values[7] = (int)_Alignof(enum gs_color_space);
	values[8] = (int)_Alignof(enum video_colorspace);
}

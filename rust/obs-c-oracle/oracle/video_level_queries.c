/* Original C video level queries and the real field adapter (#5187).
 * Excludes a dead core and concurrent writes to the levels: the original
 * queries require stable caller state. The graphics token is a real
 * graphics_t that is never dereferenced. Real core lifecycles stay covered by
 * the unchanged actual-library C tests. */
#include "libobs/build/libobs.pch.h"
#include "libobs/graphics/graphics-internal.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#if defined(_MSC_VER)
static __declspec(thread) struct obs_core *video_levels_current;
#else
static _Thread_local struct obs_core *video_levels_current;
#endif

float oracle_obs_get_video_sdr_white_level(void);
float oracle_obs_get_video_hdr_nominal_peak_level(void);
#define obs video_levels_current
#define obs_get_video_sdr_white_level oracle_obs_get_video_sdr_white_level
#define obs_get_video_hdr_nominal_peak_level oracle_obs_get_video_hdr_nominal_peak_level
#include "libobs/obs-video-level-queries.c.h"
#undef obs_get_video_hdr_nominal_peak_level
#undef obs_get_video_sdr_white_level
#include "video-level-queries-fields.inc"
#undef obs

static graphics_t video_levels_graphics;

void oracle_video_level_queries_layout(size_t *layout);
void oracle_video_level_queries_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_video_level_queries_fields);
	layout[1] = _Alignof(struct obs_video_level_queries_fields);
	layout[2] = offsetof(struct obs_video_level_queries_fields, graphics);
	layout[3] = offsetof(struct obs_video_level_queries_fields, sdr_white_level);
	layout[4] = offsetof(struct obs_video_level_queries_fields, hdr_nominal_peak_level);
}

static uint32_t video_levels_bits(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	return bits;
}

typedef float (*video_level_fn)(void);

/* core: 0 = NULL core, 1 = core without graphics, 2 = core with graphics.
 * Writes the original C results' bits to out[0..1]; true when the port
 * returns the same bits, the core bytes are unchanged and the global core
 * pointer is untouched after every call. */
bool oracle_video_level_queries_check(int mode, uint32_t white, uint32_t peak, video_level_fn white_fn,
				      video_level_fn peak_fn, uint32_t *out);
bool oracle_video_level_queries_check(int mode, uint32_t white, uint32_t peak, video_level_fn white_fn,
				      video_level_fn peak_fn, uint32_t *out)
{
	struct obs_core *core = calloc(1, sizeof(*core));
	struct obs_core *before = malloc(sizeof(*before));
	bool valid = core && before;
	if (valid) {
		memcpy(&core->video.sdr_white_level, &white, sizeof(white));
		memcpy(&core->video.hdr_nominal_peak_level, &peak, sizeof(peak));
		core->video.graphics = mode == 2 ? &video_levels_graphics : NULL;
		struct obs_core *owner = mode ? core : NULL;
		video_levels_current = owner;
		memcpy(before, core, sizeof(*core));
		out[0] = video_levels_bits(oracle_obs_get_video_sdr_white_level());
		out[1] = video_levels_bits(oracle_obs_get_video_hdr_nominal_peak_level());
		valid = memcmp(before, core, sizeof(*core)) == 0 && video_levels_current == owner;
		for (unsigned i = 0; i < 8; ++i) {
			valid = valid && video_levels_bits(white_fn()) == out[0];
			valid = valid && memcmp(before, core, sizeof(*core)) == 0 && video_levels_current == owner;
			valid = valid && video_levels_bits(peak_fn()) == out[1];
			valid = valid && memcmp(before, core, sizeof(*core)) == 0 && video_levels_current == owner;
		}
		video_levels_current = NULL;
	}
	free(before);
	free(core);
	return valid;
}

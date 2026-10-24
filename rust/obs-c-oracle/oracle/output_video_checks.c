/* Test-only oracle: the original libobs/obs-output-video-checks.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims, plus the constants and the layouts of its structs as
 * the C sees them. */
#include <stddef.h>

#define obs_output_first_mixer oracle_obs_output_first_mixer
#define obs_output_size_route oracle_obs_output_size_route
#define obs_output_capture_ready oracle_obs_output_capture_ready
#define obs_output_begin_gate oracle_obs_output_begin_gate
#define obs_output_conversion_plan oracle_obs_output_conversion_plan
#define obs_output_congestion_clamp oracle_obs_output_congestion_clamp

#include "obs-output-video-checks.c.h"

#undef obs_output_congestion_clamp
#undef obs_output_conversion_plan
#undef obs_output_begin_gate
#undef obs_output_capture_ready
#undef obs_output_size_route
#undef obs_output_first_mixer

/* In the order of rust/obs-c-oracle/src/output_video_checks.rs. */
enum {
	ORACLE_OUTPUT_VIDEO_CHECKS_CONSTANT_COUNT = 17,
	ORACLE_OUTPUT_VIDEO_CHECKS_LAYOUT_COUNT = 23,
};

long long oracle_output_video_checks_constant(size_t which);
size_t oracle_output_video_checks_layout(size_t which);

long long oracle_output_video_checks_constant(size_t which)
{
	const long long values[ORACLE_OUTPUT_VIDEO_CHECKS_CONSTANT_COUNT] = {
		OBS_OUTPUT_VIDEO_CHECKS_VIDEO,
		OBS_OUTPUT_VIDEO_CHECKS_AUDIO,
		OBS_OUTPUT_VIDEO_CHECKS_ENCODED,
		OBS_OUTPUT_VIDEO_CHECKS_SERVICE,
		OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_MIXES,
		OBS_OUTPUT_VIDEO_CHECKS_MAX_VIDEO_ENCODERS,
		OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_ENCODERS,
		OBS_OUTPUT_SIZE_ZERO,
		OBS_OUTPUT_SIZE_ENCODER,
		OBS_OUTPUT_SIZE_SCALED,
		OBS_OUTPUT_SIZE_VIDEO,
		OBS_OUTPUT_BEGIN_READY,
		OBS_OUTPUT_BEGIN_REFUSE,
		OBS_OUTPUT_BEGIN_CHECK,
		OBS_OUTPUT_CONVERSION_NONE,
		OBS_OUTPUT_CONVERSION_SET,
		OBS_OUTPUT_CONVERSION_SCALED,
	};
	return which < ORACLE_OUTPUT_VIDEO_CHECKS_CONSTANT_COUNT ? values[which] : -1;
}

size_t oracle_output_video_checks_layout(size_t which)
{
	const size_t values[ORACLE_OUTPUT_VIDEO_CHECKS_LAYOUT_COUNT] = {
		sizeof(struct obs_output_capture_presence),
		_Alignof(struct obs_output_capture_presence),
		offsetof(struct obs_output_capture_presence, flags),
		offsetof(struct obs_output_capture_presence, video_encoders),
		offsetof(struct obs_output_capture_presence, audio_encoders),
		offsetof(struct obs_output_capture_presence, video),
		offsetof(struct obs_output_capture_presence, audio),
		offsetof(struct obs_output_capture_presence, service),
		sizeof(struct obs_output_conversion_sizes),
		_Alignof(struct obs_output_conversion_sizes),
		offsetof(struct obs_output_conversion_sizes, set),
		offsetof(struct obs_output_conversion_sizes, set_width),
		offsetof(struct obs_output_conversion_sizes, set_height),
		offsetof(struct obs_output_conversion_sizes, scaled_width),
		offsetof(struct obs_output_conversion_sizes, scaled_height),
		offsetof(struct obs_output_conversion_sizes, video_width),
		offsetof(struct obs_output_conversion_sizes, video_height),
		sizeof(struct obs_output_conversion_choice),
		_Alignof(struct obs_output_conversion_choice),
		offsetof(struct obs_output_conversion_choice, kind),
		offsetof(struct obs_output_conversion_choice, width),
		offsetof(struct obs_output_conversion_choice, height),
		sizeof(enum obs_output_size_source),
	};
	return which < ORACLE_OUTPUT_VIDEO_CHECKS_LAYOUT_COUNT ? values[which] : (size_t)-1;
}

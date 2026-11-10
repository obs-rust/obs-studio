/* Test-only oracle: the original libobs/obs-video-frame-copy.c.h,
 * unmodified, with its two functions renamed to oracle_* so they can link
 * next to the Rust shims. The copy plan it calls is renamed too, to the C
 * one of oracle/video_copy_plan.c, so the whole copy is the original C. */
#include <stddef.h>
#include <stdint.h>

#define obs_video_set_gpu_converted_data oracle_obs_video_set_gpu_converted_data
#define obs_video_copy_rgbx_frame oracle_obs_video_copy_rgbx_frame
#define obs_video_gpu_copy_plan oracle_obs_video_gpu_copy_plan
#define obs_video_rgbx_copy_plan oracle_obs_video_rgbx_copy_plan

#include "obs-video-frame-copy.c.h"

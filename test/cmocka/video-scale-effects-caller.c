/* Real obs-video.c caller, isolated from OBS startup by section GC. This C
 * contract is shared by the C-reference and Rust-static boundary checks. */
struct obs_core;
extern struct obs_core *scale_effects_test_obs;
#define obs scale_effects_test_obs
#define obs_graphics_thread scale_effects_test_graphics_thread
#define obs_graphics_thread_loop scale_effects_test_graphics_thread_loop
#ifdef VIDEO_SCALE_EFFECT_CALLER_EXTRACT
#include "libobs/build/libobs.pch.h"
uint32_t obs_video_loaded_scale_effects(const struct obs_core_video *video);
gs_effect_t *obs_video_scale_effect_pointer(const struct obs_core_video *video, enum obs_video_scale_effect effect);
#include "video-scale-effects-caller.inc"
#else
#include "libobs/obs-video-frame-copy.c.h"
#include "libobs/obs-video-stop-requested.c.h"
#include "libobs/obs-video.c.h"
#endif

struct obs_core *scale_effects_test_obs;

int video_scale_effects_caller_check(void);
int video_scale_effects_caller_check(void)
{
	static gs_effect_t identities[5];
	struct obs_core core = {0};
	struct obs_core_video_mix mix = {0};
	struct obs_core_video before;
	mix.ovi.base_width = 1920;
	mix.ovi.base_height = 1080;
	scale_effects_test_obs = &core;
	for (uint32_t mask = 0; mask < 32; mask++) {
		gs_effect_t *effects[5];
		for (size_t i = 0; i < 5; i++)
			effects[i] = (mask & (1u << i)) ? &identities[i] : NULL;
		core.video.default_effect = effects[0];
		core.video.bilinear_lowres_effect = effects[1];
		core.video.bicubic_effect = effects[2];
		core.video.lanczos_effect = effects[3];
		core.video.area_effect = effects[4];
		memcpy(&before, &core.video, sizeof(before));
		if (obs_video_loaded_scale_effects(&core.video) != mask)
			return 1;
		for (int selector = 0; selector < 5; selector++)
			if (obs_video_scale_effect_pointer(&core.video, (enum obs_video_scale_effect)selector) !=
			    effects[selector])
				return 2;
		if (obs_video_scale_effect_pointer(&core.video, (enum obs_video_scale_effect)99) != effects[0])
			return 3;
		/* Close resolution takes the production get_scale_effect() path without
		 * opening a graphics device or querying a video output. */
		if (get_scale_effect(&mix, 1920, 1080) != effects[0])
			return 4;
		if (memcmp(&before, &core.video, sizeof(before)))
			return 5;
	}
	scale_effects_test_obs = NULL;
	return 0;
}

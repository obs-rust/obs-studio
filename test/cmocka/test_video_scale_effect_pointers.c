/* Pin the current C pointer adapters before their Rust port. No graphics device
 * is created: opaque pointer identities are never dereferenced. This tests
 * the C source only; it cannot exercise a Rust swap until the private symbols
 * are reachable. Keep this baseline unchanged and cover real callers too. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "libobs/obs-video-scale-effects.c.h"

static void all_effect_presence_masks(void **state)
{
	(void)state;
	static gs_effect_t identities[5];
	for (uint32_t mask = 0; mask < 32; mask++) {
		struct obs_core_video video = {0};
		gs_effect_t *effects[5];
		for (size_t i = 0; i < 5; i++)
			effects[i] = (mask & (1u << i)) ? &identities[i] : NULL;
		video.default_effect = effects[0];
		video.bilinear_lowres_effect = effects[1];
		video.bicubic_effect = effects[2];
		video.lanczos_effect = effects[3];
		video.area_effect = effects[4];
		struct obs_core_video before;
		memcpy(&before, &video, sizeof(video));
		uint32_t expected = 0;
		const enum obs_video_scale_effect selectors[] = {
			OBS_VIDEO_SCALE_EFFECT_DEFAULT, OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES,
			OBS_VIDEO_SCALE_EFFECT_BICUBIC, OBS_VIDEO_SCALE_EFFECT_LANCZOS, OBS_VIDEO_SCALE_EFFECT_AREA};
		for (size_t i = 0; i < 5; i++) {
			if (effects[i])
				expected |= OBS_VIDEO_SCALE_LOADED(selectors[i]);
			/* A missing selected effect returns NULL, not the default. */
			assert_ptr_equal(scale_effect_pointer(&video, selectors[i]), effects[i]);
		}
		assert_int_equal(loaded_scale_effects(&video), expected);
		/* Characterized, not endorsed: unknown selectors use default. */
		assert_ptr_equal(scale_effect_pointer(&video, (enum obs_video_scale_effect)99), effects[0]);
		assert_memory_equal(&video, &before, sizeof(video));
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(all_effect_presence_masks)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

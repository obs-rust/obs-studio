/* Test-only oracle: the original libobs/obs-scene-audio-actions.c.h,
 * unmodified, with its two functions renamed to oracle_* so they can link
 * next to the Rust shims. da_erase and util_mul_div64 stay the header
 * inlines. */
#include <stddef.h>

#define obs_scene_audio_actions_apply oracle_obs_scene_audio_actions_apply
#define obs_scene_audio_action_due oracle_obs_scene_audio_action_due

#include "obs-scene-audio-actions.c.h"

#undef obs_scene_audio_action_due
#undef obs_scene_audio_actions_apply

/* The layouts the C uses, in the order of
 * rust/obs-c-oracle/src/scene_audio_actions.rs. */
enum {
	ORACLE_SCENE_AUDIO_ACTIONS_LAYOUT_COUNT = 9,
};

size_t oracle_scene_audio_actions_layout(size_t which);

size_t oracle_scene_audio_actions_layout(size_t which)
{
	const size_t values[ORACLE_SCENE_AUDIO_ACTIONS_LAYOUT_COUNT] = {
		sizeof(struct item_action),
		_Alignof(struct item_action),
		offsetof(struct item_action, visible),
		offsetof(struct item_action, timestamp),
		sizeof(obs_scene_item_action_array_t),
		offsetof(obs_scene_item_action_array_t, array),
		offsetof(obs_scene_item_action_array_t, num),
		offsetof(obs_scene_item_action_array_t, capacity),
		AUDIO_OUTPUT_FRAMES,
	};
	return which < ORACLE_SCENE_AUDIO_ACTIONS_LAYOUT_COUNT ? values[which] : 0;
}

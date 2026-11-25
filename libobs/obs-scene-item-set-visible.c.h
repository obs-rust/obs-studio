#include "libobs/build/libobs.pch.h"

/* The helpers stay in the same core-source translation unit. */
static void signal_parent(obs_scene_t *parent, const char *command, calldata_t *params);
static inline bool source_has_audio(obs_source_t *source);
void obs_sceneitem_set_visibility(struct obs_scene_item *item, bool vis);

static bool group_item_transition(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	if (!param || !item)
		return true;
	const bool visible = *(bool *)param;
	if (obs_sceneitem_visible(item))
		obs_sceneitem_do_transition(item, visible);
	UNUSED_PARAMETER(scene);
	return true;
}

bool obs_sceneitem_set_visible(obs_sceneitem_t *item, bool visible)
{
	struct calldata cd;
	uint8_t stack[256];
	struct item_action action = {.visible = visible, .timestamp = os_gettime_ns()};

	if (!item)
		return false;

	if (os_atomic_load_bool(&item->user_visible) == visible)
		return false;

	if (!item->parent)
		return false;

	obs_sceneitem_do_transition(item, visible);
	if (obs_sceneitem_is_group(item))
		obs_sceneitem_group_enum_items(item, group_item_transition, &visible);

	os_atomic_set_bool(&item->user_visible, visible);

	if (visible) {
		if (os_atomic_inc_long(&item->active_refs) == 1) {
			if (!obs_source_add_active_child(item->parent->source, item->source)) {
				os_atomic_dec_long(&item->active_refs);
				return false;
			}
		}
	}

	calldata_init_fixed(&cd, stack, sizeof(stack));
	calldata_set_ptr(&cd, "item", item);
	calldata_set_bool(&cd, "visible", visible);

	signal_parent(item->parent, "item_visible", &cd);

	if (source_has_audio(item->source)) {
		pthread_mutex_lock(&item->actions_mutex);
		da_push_back(item->audio_actions, &action);
		pthread_mutex_unlock(&item->actions_mutex);
	} else {
		obs_sceneitem_set_visibility(item, visible);
	}
	return true;
}

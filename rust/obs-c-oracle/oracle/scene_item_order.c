/* Test-only oracle: the original libobs/obs-scene-item-order.c.h,
 * unmodified, with its ten functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>

#define obs_scene_item_order_anchor oracle_obs_scene_item_order_anchor
#define obs_scene_item_order_position oracle_obs_scene_item_order_position
#define obs_scene_item_order_item_attached oracle_obs_scene_item_order_item_attached
#define obs_scene_item_order_match_item oracle_obs_scene_item_order_match_item
#define obs_scene_item_order_reorder_check oracle_obs_scene_item_order_reorder_check
#define obs_scene_item_order_in_group_run oracle_obs_scene_item_order_in_group_run
#define obs_scene_item_order_add_item_allowed oracle_obs_scene_item_order_add_item_allowed
#define obs_scene_item_order_remove_item_allowed oracle_obs_scene_item_order_remove_item_allowed
#define obs_scene_item_order_moves_scene oracle_obs_scene_item_order_moves_scene
#define obs_scene_item_order_item_active oracle_obs_scene_item_order_item_active

#include "obs-scene-item-order.c.h"

#undef obs_scene_item_order_item_active
#undef obs_scene_item_order_moves_scene
#undef obs_scene_item_order_remove_item_allowed
#undef obs_scene_item_order_add_item_allowed
#undef obs_scene_item_order_in_group_run
#undef obs_scene_item_order_reorder_check
#undef obs_scene_item_order_match_item
#undef obs_scene_item_order_item_attached
#undef obs_scene_item_order_position
#undef obs_scene_item_order_anchor

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/scene_item_order.rs. */
enum {
	ORACLE_SCENE_ITEM_ORDER_LAYOUT_COUNT = 20,
};

size_t oracle_scene_item_order_layout(size_t which);

size_t oracle_scene_item_order_layout(size_t which)
{
	const size_t values[ORACLE_SCENE_ITEM_ORDER_LAYOUT_COUNT] = {
		sizeof(struct obs_sceneitem_order_info),
		_Alignof(struct obs_sceneitem_order_info),
		offsetof(struct obs_sceneitem_order_info, group),
		offsetof(struct obs_sceneitem_order_info, item),
		sizeof(enum obs_order_movement),
		OBS_ORDER_MOVE_UP,
		OBS_ORDER_MOVE_DOWN,
		OBS_ORDER_MOVE_TOP,
		OBS_ORDER_MOVE_BOTTOM,
		OBS_SCENE_ITEM_ORDER_ANCHOR_NONE,
		OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM,
		OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV,
		OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT,
		OBS_SCENE_ITEM_ORDER_ANCHOR_PREV,
		OBS_SCENE_ITEM_ORDER_ANCHOR_LAST,
		OBS_SCENE_ITEM_ORDER_ANCHOR_STAY,
		OBS_SCENE_ITEM_ORDER_SAME,
		OBS_SCENE_ITEM_ORDER_APPLY,
		OBS_SCENE_ITEM_ORDER_INVALID,
		sizeof(size_t),
	};
	return which < ORACLE_SCENE_ITEM_ORDER_LAYOUT_COUNT ? values[which] : 0;
}

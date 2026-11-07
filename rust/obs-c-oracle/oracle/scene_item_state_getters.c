/* Original extracted C getters, renamed and compiled with the real headers. */
#define obs_sceneitem_selected oracle_obs_sceneitem_selected
#define obs_sceneitem_locked oracle_obs_sceneitem_locked
#include "obs-scene-item-state-getters.c.h"
#undef obs_sceneitem_selected
#undef obs_sceneitem_locked

typedef bool (*scene_state_getter)(const obs_sceneitem_t *);
size_t oracle_scene_item_state_layout(unsigned field);
unsigned oracle_scene_item_state_probe(unsigned flags, int64_t id, scene_state_getter selected,
				       scene_state_getter locked);

size_t oracle_scene_item_state_layout(unsigned field)
{
	/* This is a prefix, not a replacement for the complete item. Its trailing
     * padding ends at item_render. Its alignment is the maximum of the real
     * member types (bool, long, int64_t and pointers), not matrix4's alignment
     * in the opaque suffix. */
	size_t align = _Alignof(long);
	if (align < _Alignof(int64_t))
		align = _Alignof(int64_t);
	if (align < _Alignof(void *))
		align = _Alignof(void *);
	switch (field) {
	case 0:
		return offsetof(struct obs_scene_item, item_render);
	case 1:
		return align;
	case 2:
		return offsetof(struct obs_scene_item, ref);
	case 3:
		return offsetof(struct obs_scene_item, removed);
	case 4:
		return offsetof(struct obs_scene_item, is_group);
	case 5:
		return offsetof(struct obs_scene_item, is_scene);
	case 6:
		return offsetof(struct obs_scene_item, update_transform);
	case 7:
		return offsetof(struct obs_scene_item, update_group_resize);
	case 8:
		return offsetof(struct obs_scene_item, id);
	case 9:
		return offsetof(struct obs_scene_item, parent);
	case 10:
		return offsetof(struct obs_scene_item, source);
	case 11:
		return offsetof(struct obs_scene_item, active_refs);
	case 12:
		return offsetof(struct obs_scene_item, defer_update);
	case 13:
		return offsetof(struct obs_scene_item, defer_group_resize);
	case 14:
		return offsetof(struct obs_scene_item, user_visible);
	case 15:
		return offsetof(struct obs_scene_item, visible);
	case 16:
		return offsetof(struct obs_scene_item, selected);
	case 17:
		return offsetof(struct obs_scene_item, locked);
	default:
		return (size_t)-1;
	}
}

unsigned oracle_scene_item_state_probe(unsigned flags, int64_t id, scene_state_getter selected,
				       scene_state_getter locked)
{
	struct obs_scene_item item = {0};
	item.selected = (flags & 1) != 0;
	item.locked = (flags & 2) != 0;
	item.visible = (flags & 4) != 0;
	item.user_visible = (flags & 8) != 0;
	item.removed = (flags & 16) != 0;
	item.is_group = (flags & 32) != 0;
	item.is_scene = (flags & 64) != 0;
	item.update_transform = (flags & 128) != 0;
	item.update_group_resize = (flags & 256) != 0;
	item.ref = (long)id;
	item.active_refs = (long)~id;
	item.defer_update = 17;
	item.defer_group_resize = -19;
	item.id = id;
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	unsigned result = 0;
	if (selected(NULL) != oracle_obs_sceneitem_selected(NULL))
		result |= 1;
	if (locked(NULL) != oracle_obs_sceneitem_locked(NULL))
		result |= 2;
	if (selected(&item) != oracle_obs_sceneitem_selected(&item))
		result |= 4;
	if (locked(&item) != oracle_obs_sceneitem_locked(&item))
		result |= 8;
	if (memcmp(before, &item, sizeof(item)))
		result |= 16;
	return result;
}

/* Verbatim extracted getter with the real private scene item header. */
#define obs_sceneitem_get_id oracle_obs_sceneitem_get_id
#include "obs-scene-item-id.c.h"
#undef obs_sceneitem_get_id

typedef int64_t (*scene_id_getter)(const obs_sceneitem_t *);
size_t oracle_scene_item_id_layout(unsigned field);
unsigned oracle_scene_item_id_probe(unsigned flags, int64_t id, scene_id_getter getter);
unsigned oracle_scene_item_id_null_probe(scene_id_getter getter);

size_t oracle_scene_item_id_layout(unsigned field)
{
	size_t align = _Alignof(long);
	if (align < _Alignof(int64_t))
		align = _Alignof(int64_t);
	switch (field) {
	case 0:
		return offsetof(struct obs_scene_item, parent);
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
	default:
		return (size_t)-1;
	}
}

unsigned oracle_scene_item_id_probe(unsigned flags, int64_t id, scene_id_getter getter)
{
	struct obs_scene_item item = {0};
	item.ref = -31;
	item.id = id;
	item.selected = (flags & 1) != 0;
	item.locked = (flags & 2) != 0;
	item.removed = (flags & 4) != 0;
	item.is_group = (flags & 8) != 0;
	item.is_scene = (flags & 16) != 0;
	item.update_transform = (flags & 32) != 0;
	item.update_group_resize = (flags & 64) != 0;
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	unsigned result = 0;
	if (getter(&item) != oracle_obs_sceneitem_get_id(&item))
		result |= 1;
	if (memcmp(before, &item, sizeof(item)))
		result |= 2;
	return result;
}

struct scene_id_log {
	unsigned count;
	int level;
	char message[128];
};

/* va_list printf callback, matching the format-checked log-capture oracles. */
#if defined(__GNUC__) || defined(__clang__)
static void scene_id_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void scene_id_capture(int level, const char *format, va_list args, void *param)
{
	struct scene_id_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

/* The Rust test re-execs before changing the process-wide log handler. */
unsigned oracle_scene_item_id_null_probe(scene_id_getter getter)
{
	log_handler_t previous;
	void *param;
	struct scene_id_log c_log = {0}, rust_log = {0};
	base_get_log_handler(&previous, &param);
	base_set_log_handler(scene_id_capture, &c_log);
	int64_t c_id = oracle_obs_sceneitem_get_id(NULL);
	base_set_log_handler(scene_id_capture, &rust_log);
	int64_t rust_id = getter(NULL);
	base_set_log_handler(previous, param);
	unsigned result = 0;
	if (c_id != 0 || rust_id != c_id)
		result |= 1;
	if (c_log.count != 1 || rust_log.count != c_log.count)
		result |= 2;
	if (c_log.level != LOG_DEBUG || rust_log.level != c_log.level)
		result |= 4;
	if (strcmp(c_log.message, "obs_sceneitem_get_id: Null 'item' parameter") ||
	    strcmp(c_log.message, rust_log.message))
		result |= 8;
	return result;
}

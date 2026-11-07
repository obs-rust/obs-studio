/* Original C getters and full real-header scene item fixtures. */
#define obs_sceneitem_get_scale_filter oracle_obs_sceneitem_get_scale_filter
#define obs_sceneitem_get_blending_method oracle_obs_sceneitem_get_blending_method
#define obs_sceneitem_get_blending_mode oracle_obs_sceneitem_get_blending_mode
#include "obs-scene-item-render-getters.c.h"
#undef obs_sceneitem_get_scale_filter
#undef obs_sceneitem_get_blending_method
#undef obs_sceneitem_get_blending_mode

typedef unsigned (*render_getter)(obs_sceneitem_t *);
size_t oracle_scene_render_layout(unsigned field);
unsigned oracle_scene_render_probe(unsigned scale, unsigned method, unsigned mode, unsigned flags,
				   render_getter scale_get, render_getter method_get, render_getter mode_get);
unsigned oracle_scene_render_null_probe(render_getter scale_get, render_getter method_get, render_getter mode_get);

size_t oracle_scene_render_layout(unsigned field)
{
	size_t align = _Alignof(long);
	if (align < _Alignof(int64_t))
		align = _Alignof(int64_t);
	if (align < _Alignof(void *))
		align = _Alignof(void *);
	size_t end = offsetof(struct obs_scene_item, blend_type) + sizeof(((struct obs_scene_item *)0)->blend_type);
	switch (field) {
	case 0:
		return (end + align - 1) / align * align;
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
	case 18:
		return offsetof(struct obs_scene_item, item_render);
	case 19:
		return offsetof(struct obs_scene_item, crop);
	case 20:
		return offsetof(struct obs_scene_item, absolute_coordinates);
	case 21:
		return offsetof(struct obs_scene_item, pos);
	case 22:
		return offsetof(struct obs_scene_item, scale);
	case 23:
		return offsetof(struct obs_scene_item, scale_ref);
	case 24:
		return offsetof(struct obs_scene_item, rot);
	case 25:
		return offsetof(struct obs_scene_item, align);
	case 26:
		return offsetof(struct obs_scene_item, last_width);
	case 27:
		return offsetof(struct obs_scene_item, last_height);
	case 28:
		return offsetof(struct obs_scene_item, output_scale);
	case 29:
		return offsetof(struct obs_scene_item, scale_filter);
	case 30:
		return offsetof(struct obs_scene_item, blend_method);
	case 31:
		return offsetof(struct obs_scene_item, blend_type);
	case 32:
		return sizeof(enum obs_scale_type);
	case 33:
		return _Alignof(enum obs_scale_type);
	case 34:
		return sizeof(enum obs_blending_method);
	case 35:
		return _Alignof(enum obs_blending_method);
	case 36:
		return sizeof(enum obs_blending_type);
	case 37:
		return _Alignof(enum obs_blending_type);
	case 38:
		return sizeof(struct obs_sceneitem_crop);
	case 39:
		return _Alignof(struct obs_sceneitem_crop);
	case 40:
		return sizeof(struct vec2);
	case 41:
		return _Alignof(struct vec2);
	default:
		return (size_t)-1;
	}
}

unsigned oracle_scene_render_probe(unsigned scale, unsigned method, unsigned mode, unsigned flags,
				   render_getter scale_get, render_getter method_get, render_getter mode_get)
{
	struct obs_scene_item item = {0};
	item.scale_filter = (enum obs_scale_type)scale;
	item.blend_method = (enum obs_blending_method)method;
	item.blend_type = (enum obs_blending_type)mode;
	item.id = -17;
	item.removed = (flags & 1) != 0;
	item.selected = (flags & 2) != 0;
	item.locked = (flags & 4) != 0;
	item.absolute_coordinates = (flags & 8) != 0;
	item.pos.x = 13.5f;
	item.scale.y = -7.0f;
	item.crop.left = 19;
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	unsigned result = 0;
	if (scale_get(&item) != (unsigned)oracle_obs_sceneitem_get_scale_filter(&item))
		result |= 1;
	if (method_get(&item) != (unsigned)oracle_obs_sceneitem_get_blending_method(&item))
		result |= 2;
	if (mode_get(&item) != (unsigned)oracle_obs_sceneitem_get_blending_mode(&item))
		result |= 4;
	if (memcmp(before, &item, sizeof(item)))
		result |= 8;
	return result;
}

struct render_log {
	unsigned count;
	int level;
	char text[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void render_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void render_capture(int level, const char *format, va_list args, void *param)
{
	struct render_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->text, sizeof(log->text), format, args);
}

unsigned oracle_scene_render_null_probe(render_getter scale_get, render_getter method_get, render_getter mode_get)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	render_getter getters[] = {scale_get, method_get, mode_get};
	unsigned result = 0;
	for (unsigned i = 0; i < 3; i++) {
		struct render_log c_log = {0}, rust_log = {0};
		base_set_log_handler(render_capture, &c_log);
		unsigned expected;
		if (i == 0)
			expected = (unsigned)oracle_obs_sceneitem_get_scale_filter(NULL);
		else if (i == 1)
			expected = (unsigned)oracle_obs_sceneitem_get_blending_method(NULL);
		else
			expected = (unsigned)oracle_obs_sceneitem_get_blending_mode(NULL);
		base_set_log_handler(render_capture, &rust_log);
		if (getters[i](NULL) != expected)
			result |= 1;
		if (c_log.count != 1 || rust_log.count != c_log.count || rust_log.level != c_log.level ||
		    strcmp(rust_log.text, c_log.text))
			result |= 2;
	}
	base_set_log_handler(previous, param);
	return result;
}

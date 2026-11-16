/* Verbatim original setter and complete real-header fixtures; no exclusions. */
#define obs_sceneitem_set_blending_mode oracle_obs_sceneitem_set_blending_mode
#include "obs-scene-item-blending-mode.c.h"
#undef obs_sceneitem_set_blending_mode
#include "scene_item_blending_mode_fields.inc"

typedef void (*blend_setter)(obs_sceneitem_t *, unsigned);
unsigned oracle_scene_item_blending_mode_probe(unsigned initial, unsigned value, unsigned flags, blend_setter setter);
size_t oracle_scene_item_blending_mode_layout(unsigned field);
unsigned oracle_scene_item_blending_mode_null(blend_setter setter);

size_t oracle_scene_item_blending_mode_layout(unsigned field)
{
	struct blend_enum_alignment {
		char prefix;
		enum obs_blending_type value;
	};
	switch (field) {
	case 0:
		return sizeof(enum obs_blending_type);
	case 1:
		return offsetof(struct blend_enum_alignment, value);
	default:
		return sizeof(struct obs_scene_item);
	}
}

unsigned oracle_scene_item_blending_mode_probe(unsigned initial, unsigned value, unsigned flags, blend_setter setter)
{
	struct obs_scene_item expected = {0}, actual;
	expected.blend_type = (enum obs_blending_type)initial;
	expected.blend_method = OBS_BLEND_METHOD_SRGB_OFF;
	expected.scale_filter = OBS_SCALE_LANCZOS;
	expected.id = -17;
	expected.selected = (flags & 1) != 0;
	expected.locked = (flags & 2) != 0;
	expected.removed = (flags & 4) != 0;
	os_atomic_store_bool(&expected.update_transform, (flags & 8) != 0);
	os_atomic_store_bool(&expected.update_group_resize, (flags & 16) != 0);
	memcpy(&actual, &expected, sizeof(actual));
	unsigned result = obs_sceneitem_blending_mode_field(&actual) != (void *)&actual.blend_type;
	oracle_obs_sceneitem_set_blending_mode(&expected, (enum obs_blending_type)value);
	setter(&actual, value);
	if (memcmp(&actual, &expected, sizeof(actual)))
		result |= 2;
	if ((unsigned)actual.blend_type != value || !os_atomic_load_bool(&actual.update_transform))
		result |= 4;
	/* Even a repeated mode dirties the transform after it has been consumed. */
	os_atomic_store_bool(&expected.update_transform, false);
	os_atomic_store_bool(&actual.update_transform, false);
	oracle_obs_sceneitem_set_blending_mode(&expected, (enum obs_blending_type)value);
	setter(&actual, value);
	if (memcmp(&actual, &expected, sizeof(actual)))
		result |= 8;
	return result;
}

struct blend_log {
	unsigned count;
	int level;
	char text[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void blend_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void blend_capture(int level, const char *format, va_list args, void *param)
{
	struct blend_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->text, sizeof(log->text), format, args);
}

unsigned oracle_scene_item_blending_mode_null(blend_setter setter)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	struct blend_log expected = {0}, actual = {0};
	base_set_log_handler(blend_capture, &expected);
	oracle_obs_sceneitem_set_blending_mode(NULL, OBS_BLEND_DARKEN);
	base_set_log_handler(blend_capture, &actual);
	setter(NULL, OBS_BLEND_DARKEN);
	base_set_log_handler(previous, param);
	return expected.count != 1 || actual.count != expected.count || actual.level != expected.level ||
	       strcmp(actual.text, expected.text) != 0;
}

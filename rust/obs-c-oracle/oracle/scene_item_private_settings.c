/* Verbatim extracted getter with the real private scene item header. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_sceneitem_get_private_settings oracle_obs_sceneitem_get_private_settings
#include "obs-sceneitem-private-settings.c.h"
#undef obs_sceneitem_get_private_settings

size_t oracle_scene_item_private_settings_layout(unsigned field);
void *oracle_scene_item_private_settings_new(obs_data_t *settings, unsigned char fill);
bool oracle_scene_item_private_settings_unchanged(const void *handle);
void oracle_scene_item_private_settings_free(void *handle);

size_t oracle_scene_item_private_settings_layout(unsigned field)
{
	const size_t alignment = _Alignof(struct matrix4);
	const size_t span = offsetof(struct obs_scene_item, private_settings) + sizeof(obs_data_t *);
	switch (field) {
	case 0:
		return (span + alignment - 1) / alignment * alignment;
	case 1:
		return alignment;
	case 2:
		return offsetof(struct obs_scene_item, ref);
	case 3:
		return offsetof(struct obs_scene_item, item_render);
	case 4:
		return offsetof(struct obs_scene_item, crop);
	case 5:
		return offsetof(struct obs_scene_item, absolute_coordinates);
	case 6:
		return offsetof(struct obs_scene_item, pos);
	case 7:
		return offsetof(struct obs_scene_item, scale);
	case 8:
		return offsetof(struct obs_scene_item, scale_ref);
	case 9:
		return offsetof(struct obs_scene_item, rot);
	case 10:
		return offsetof(struct obs_scene_item, align);
	case 11:
		return offsetof(struct obs_scene_item, last_width);
	case 12:
		return offsetof(struct obs_scene_item, last_height);
	case 13:
		return offsetof(struct obs_scene_item, output_scale);
	case 14:
		return offsetof(struct obs_scene_item, scale_filter);
	case 15:
		return offsetof(struct obs_scene_item, blend_method);
	case 16:
		return offsetof(struct obs_scene_item, blend_type);
	case 17:
		return offsetof(struct obs_scene_item, box_transform);
	case 18:
		return offsetof(struct obs_scene_item, box_scale);
	case 19:
		return offsetof(struct obs_scene_item, draw_transform);
	case 20:
		return offsetof(struct obs_scene_item, bounds_type);
	case 21:
		return offsetof(struct obs_scene_item, bounds_align);
	case 22:
		return offsetof(struct obs_scene_item, bounds);
	case 23:
		return offsetof(struct obs_scene_item, crop_to_bounds);
	case 24:
		return offsetof(struct obs_scene_item, bounds_crop);
	case 25:
		return offsetof(struct obs_scene_item, toggle_visibility);
	case 26:
		return offsetof(struct obs_scene_item, private_settings);
	case 27:
		return sizeof(obs_hotkey_pair_id);
	case 28:
		return sizeof(struct obs_sceneitem_crop);
	default:
		return (size_t)-1;
	}
}

/* A complete item whose every byte, padding and suffix included, is the fill
 * pattern except the stored settings; the copy detects any write. */
struct scene_item_private_settings_fixture {
	struct obs_scene_item item;
	struct obs_scene_item before;
};

void *oracle_scene_item_private_settings_new(obs_data_t *settings, unsigned char fill)
{
	struct scene_item_private_settings_fixture *fixture = malloc(sizeof(*fixture));
	if (fixture) {
		memset(fixture, fill, sizeof(*fixture));
		fixture->item.private_settings = settings;
		memcpy(&fixture->before, &fixture->item, sizeof(fixture->item));
	}
	return fixture;
}

bool oracle_scene_item_private_settings_unchanged(const void *handle)
{
	const struct scene_item_private_settings_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->item, sizeof(fixture->item)) == 0;
}

void oracle_scene_item_private_settings_free(void *handle)
{
	/* The Rust test owns the settings object and every returned reference. */
	free(handle);
}

struct scene_item_private_settings_log {
	unsigned count;
	int level;
	char message[160];
};

/* va_list printf callback, matching the format-checked log-capture oracles. */
#if defined(__GNUC__) || defined(__clang__)
static void scene_item_private_settings_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void scene_item_private_settings_capture(int level, const char *format, va_list args, void *param)
{
	struct scene_item_private_settings_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef obs_data_t *(*scene_item_private_settings_getter)(obs_sceneitem_t *);
unsigned oracle_scene_item_private_settings_null_probe(scene_item_private_settings_getter getter);

/* The Rust test re-execs before changing the process-wide log handler. */
unsigned oracle_scene_item_private_settings_null_probe(scene_item_private_settings_getter getter)
{
	log_handler_t previous;
	void *param;
	struct scene_item_private_settings_log c_log = {0}, rust_log = {0};
	base_get_log_handler(&previous, &param);
	base_set_log_handler(scene_item_private_settings_capture, &c_log);
	obs_data_t *c_value = oracle_obs_sceneitem_get_private_settings(NULL);
	base_set_log_handler(scene_item_private_settings_capture, &rust_log);
	obs_data_t *rust_value = getter(NULL);
	base_set_log_handler(previous, param);
	unsigned result = 0;
	if (c_value || rust_value)
		result |= 1;
	if (c_log.count != 1 || rust_log.count != c_log.count)
		result |= 2;
	if (c_log.level != LOG_DEBUG || rust_log.level != c_log.level)
		result |= 4;
	if (strcmp(c_log.message, "obs_sceneitem_get_private_settings: Null 'item' parameter") ||
	    strcmp(c_log.message, rust_log.message))
		result |= 8;
	return result;
}

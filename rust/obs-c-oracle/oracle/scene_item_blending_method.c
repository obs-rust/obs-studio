/* Exact original setter; Rust is reached only through the supplied callback. */
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include "util/threading.h"
#include "obs-scene.h"
void oracle_obs_sceneitem_set_blending_method(obs_sceneitem_t *item, enum obs_blending_method method);
#define obs_sceneitem_set_blending_method oracle_obs_sceneitem_set_blending_method
#include "obs-scene-item-blending-method.c.h"
#undef obs_sceneitem_set_blending_method
#include "scene_item_blending_method_fields.inc"

typedef void (*method_setter)(obs_sceneitem_t *, enum obs_blending_method);
unsigned oracle_scene_item_blending_method_probe(unsigned method, unsigned flags, method_setter setter);
unsigned oracle_scene_item_blending_method_null_probe(method_setter setter);
size_t oracle_scene_item_blending_method_size(void);

size_t oracle_scene_item_blending_method_size(void)
{
	return sizeof(enum obs_blending_method);
}

unsigned oracle_scene_item_blending_method_probe(unsigned method, unsigned flags, method_setter setter)
{
	struct obs_scene_item item = {0}, expected;
	item.id = -17;
	item.removed = true;
	item.selected = true;
	item.scale_filter = OBS_SCALE_BICUBIC;
	item.blend_type = OBS_BLEND_ADDITIVE;
	os_atomic_set_bool(&item.update_transform, (flags & 1) != 0);
	const unsigned values[] = {0, 1, 17, 0x7fffffff, method};
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		for (unsigned repeat = 0; repeat < 2; repeat++) {
			memcpy(&expected, &item, sizeof(item));
			oracle_obs_sceneitem_set_blending_method(&expected, (enum obs_blending_method)values[i]);
			setter(&item, (enum obs_blending_method)values[i]);
			if (memcmp(&expected, &item, sizeof(item)))
				return 1;
		}
	}
	return 0;
}

struct method_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void method_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void method_capture(int level, const char *format, va_list args, void *param)
{
	struct method_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

unsigned oracle_scene_item_blending_method_null_probe(method_setter setter)
{
	log_handler_t previous;
	void *param;
	struct method_log c_log = {0}, rust_log = {0};
	base_get_log_handler(&previous, &param);
	base_set_log_handler(method_capture, &c_log);
	oracle_obs_sceneitem_set_blending_method(NULL, OBS_BLEND_METHOD_DEFAULT);
	base_set_log_handler(method_capture, &rust_log);
	setter(NULL, OBS_BLEND_METHOD_DEFAULT);
	base_set_log_handler(previous, param);
	return c_log.count != 1 || rust_log.count != c_log.count || c_log.level != LOG_DEBUG ||
	       rust_log.level != c_log.level ||
	       strcmp(c_log.message, "obs_sceneitem_set_blending_method: Null 'item' parameter") ||
	       strcmp(c_log.message, rust_log.message);
}

/* Exact original setter, complete-item byte parity and null diagnostic parity. */
#define obs_sceneitem_set_scale_filter oracle_obs_sceneitem_set_scale_filter
#include "obs-scene-item-scale-filter-setter.c.h"
#undef obs_sceneitem_set_scale_filter

typedef void (*scale_filter_setter)(obs_sceneitem_t *, enum obs_scale_type);
unsigned oracle_scene_scale_filter_probe(unsigned initial, unsigned filter, bool dirty, scale_filter_setter setter);
unsigned oracle_scene_scale_filter_null_probe(scale_filter_setter setter);

unsigned oracle_scene_scale_filter_probe(unsigned initial, unsigned filter, bool dirty, scale_filter_setter setter)
{
	struct obs_scene_item expected, actual;
	memset(&expected, 0, sizeof(expected));
	expected.scale_filter = (enum obs_scale_type)initial;
	expected.update_transform = dirty;
	expected.id = -17;
	expected.pos.x = 13.5f;
	expected.crop.left = 19;
	expected.blend_method = OBS_BLEND_METHOD_SRGB_OFF;
	expected.selected = true;
	memcpy(&actual, &expected, sizeof(actual));
	oracle_obs_sceneitem_set_scale_filter(&expected, (enum obs_scale_type)filter);
	setter(&actual, (enum obs_scale_type)filter);
	return memcmp(&actual, &expected, sizeof(actual)) != 0;
}

struct scale_filter_log {
	unsigned count;
	int level;
	char text[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void scale_filter_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void scale_filter_capture(int level, const char *format, va_list args, void *param)
{
	struct scale_filter_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->text, sizeof(log->text), format, args);
}

unsigned oracle_scene_scale_filter_null_probe(scale_filter_setter setter)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	struct scale_filter_log expected = {0}, actual = {0};
	base_set_log_handler(scale_filter_capture, &expected);
	oracle_obs_sceneitem_set_scale_filter(NULL, OBS_SCALE_AREA);
	base_set_log_handler(scale_filter_capture, &actual);
	setter(NULL, OBS_SCALE_AREA);
	base_set_log_handler(previous, param);
	return expected.count != 1 || actual.count != expected.count || actual.level != expected.level ||
	       strcmp(actual.text, expected.text) != 0;
}

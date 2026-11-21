/* Original extracted C and the real production atomic adapters. The bounds
 * oracle owns the shared transform recorder and its complete native items. */
#define obs_sceneitem_defer_update_begin oracle_obs_sceneitem_defer_update_begin
#define obs_sceneitem_defer_update_end oracle_obs_sceneitem_defer_update_end
#include "libobs/build/libobs.pch.h"
#include "sceneitem-defer-update.inc"
#undef obs_sceneitem_defer_update_begin
#undef obs_sceneitem_defer_update_end
#include "sceneitem_defer_update_fields.inc"

typedef void (*defer_operation)(obs_sceneitem_t *);
unsigned oracle_scene_bounds_defer_probe(long initial, unsigned flags, const unsigned char *ops, size_t count,
					 defer_operation c_begin, defer_operation c_end, defer_operation begin,
					 defer_operation end);
unsigned oracle_sceneitem_defer_probe(long initial, unsigned flags, const unsigned char *ops, size_t count,
				      defer_operation begin, defer_operation end);
unsigned oracle_sceneitem_defer_null(defer_operation begin, defer_operation end);

unsigned oracle_sceneitem_defer_probe(long initial, unsigned flags, const unsigned char *ops, size_t count,
				      defer_operation begin, defer_operation end)
{
	return oracle_scene_bounds_defer_probe(initial, flags, ops, count, oracle_obs_sceneitem_defer_update_begin,
					       oracle_obs_sceneitem_defer_update_end, begin, end);
}

struct defer_log {
	unsigned count;
	int level;
	char text[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void defer_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void defer_capture(int level, const char *format, va_list args, void *param)
{
	struct defer_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->text, sizeof(log->text), format, args);
}

unsigned oracle_sceneitem_defer_null(defer_operation begin, defer_operation end)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	defer_operation c[] = {oracle_obs_sceneitem_defer_update_begin, oracle_obs_sceneitem_defer_update_end};
	defer_operation rust[] = {begin, end};
	const char *messages[] = {"obs_sceneitem_defer_update_begin: Null 'item' parameter",
				  "obs_sceneitem_defer_update_end: Null 'item' parameter"};
	unsigned result = 0;
	for (unsigned i = 0; i < 2; i++) {
		struct defer_log expected = {0}, actual = {0};
		base_set_log_handler(defer_capture, &expected);
		c[i](NULL);
		base_set_log_handler(defer_capture, &actual);
		rust[i](NULL);
		if (expected.count != 1 || expected.level != LOG_DEBUG || strcmp(expected.text, messages[i]) ||
		    actual.count != expected.count || actual.level != expected.level ||
		    strcmp(actual.text, expected.text))
			result |= 1u << i;
	}
	base_set_log_handler(previous, param);
	return result;
}

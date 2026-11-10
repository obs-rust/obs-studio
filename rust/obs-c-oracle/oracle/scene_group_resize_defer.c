/* Verbatim C baseline and the real field adapter. No mirrored item layout. */
#include <stdio.h>
#include <string.h>
#define obs_sceneitem_defer_group_resize_begin oracle_obs_sceneitem_defer_group_resize_begin
#define obs_sceneitem_defer_group_resize_end oracle_obs_sceneitem_defer_group_resize_end
#include "libobs/obs-scene-item-group-resize-defer.c.h"
#undef obs_sceneitem_defer_group_resize_begin
#undef obs_sceneitem_defer_group_resize_end
#include "scene_group_resize_defer_fields.inc"

typedef void (*resize_operation)(obs_sceneitem_t *);
unsigned oracle_scene_group_resize_defer_probe(int initial, bool flag, const unsigned char *ops, size_t count,
					       resize_operation begin, resize_operation end);
unsigned oracle_scene_group_resize_defer_null(resize_operation begin, resize_operation end);

unsigned oracle_scene_group_resize_defer_probe(int initial, bool flag, const unsigned char *ops, size_t count,
					       resize_operation begin, resize_operation end)
{
	struct obs_scene_item expected, actual;
	struct obs_scene scene;
	memset(&scene, 0, sizeof(scene));
	if (pthread_mutex_init(&scene.video_mutex, NULL) != 0)
		return 1000;
	/* Calls are legal while the caller already owns the nonrecursive scene
	 * mutex: adding an item-fields lock here would deadlock. */
	pthread_mutex_lock(&scene.video_mutex);
	memset(&expected, 0, sizeof(expected));
	expected.id = -37;
	expected.parent = &scene;
	expected.selected = true;
	expected.defer_update = 19;
	os_atomic_store_long(&expected.defer_group_resize, initial);
	os_atomic_store_bool(&expected.update_group_resize, flag);
	memcpy(&actual, &expected, sizeof(actual));
	unsigned result = 0;
	for (size_t i = 0; i < count; i++) {
		if (ops[i]) {
			oracle_obs_sceneitem_defer_group_resize_begin(&expected);
			begin(&actual);
		} else {
			oracle_obs_sceneitem_defer_group_resize_end(&expected);
			end(&actual);
		}
		/* Real-header field coverage plus every untouched byte (including the
		 * parent, source, defer_update and transform fields). Calls must not
		 * follow parent/source pointers or trigger transform work. */
		if (memcmp(&actual, &expected, sizeof(actual))) {
			result = (unsigned)i + 1;
			break;
		}
	}
	pthread_mutex_unlock(&scene.video_mutex);
	pthread_mutex_destroy(&scene.video_mutex);
	return result;
}

struct resize_log {
	unsigned count;
	int level;
	char text[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void resize_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void resize_capture(int level, const char *format, va_list args, void *param)
{
	struct resize_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->text, sizeof(log->text), format, args);
}

unsigned oracle_scene_group_resize_defer_null(resize_operation begin, resize_operation end)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	resize_operation oracle[] = {oracle_obs_sceneitem_defer_group_resize_begin,
				     oracle_obs_sceneitem_defer_group_resize_end};
	resize_operation rust[] = {begin, end};
	unsigned result = 0;
	for (unsigned i = 0; i < 2; i++) {
		struct resize_log expected = {0}, actual = {0};
		base_set_log_handler(resize_capture, &expected);
		oracle[i](NULL);
		base_set_log_handler(resize_capture, &actual);
		rust[i](NULL);
		if (expected.count != 1 || actual.count != expected.count || actual.level != expected.level ||
		    strcmp(actual.text, expected.text))
			result |= 1u << i;
	}
	base_set_log_handler(previous, param);
	return result;
}

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-scene.h"

struct captured_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct captured_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static void null_item(void **state)
{
	(void)state;
	log_handler_t previous;
	void *param;
	struct captured_log log = {0};
	base_get_log_handler(&previous, &param);
	base_set_log_handler(capture_log, &log);
	obs_sceneitem_set_blending_method(NULL, OBS_BLEND_METHOD_DEFAULT);
	base_set_log_handler(previous, param);
	assert_int_equal(log.count, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_sceneitem_set_blending_method: Null 'item' parameter");
}

/* Unknown nonnegative enum values are stored unchanged: characterized, not
 * endorsed. Unlike the blending mode setter, this never dirties the transform. */
static void stored_values(void **state)
{
	(void)state;
	const unsigned values[] = {OBS_BLEND_METHOD_DEFAULT, OBS_BLEND_METHOD_SRGB_OFF, 17, 0x7fffffff};
	for (unsigned dirty = 0; dirty < 2; dirty++) {
		struct obs_scene_item item = {0};
		item.id = -17;
		item.removed = true;
		item.selected = true;
		item.scale_filter = OBS_SCALE_BICUBIC;
		item.blend_type = OBS_BLEND_ADDITIVE;
		os_atomic_set_bool(&item.update_transform, dirty != 0);
		for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
			for (unsigned repeat = 0; repeat < 2; repeat++) {
				struct obs_scene_item expected;
				memcpy(&expected, &item, sizeof(item));
				expected.blend_method = (enum obs_blending_method)values[i];
				obs_sceneitem_set_blending_method(&item, (enum obs_blending_method)values[i]);
				assert_memory_equal(&expected, &item, sizeof(item));
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_item), cmocka_unit_test(stored_values)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

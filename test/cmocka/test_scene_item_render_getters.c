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

static void null_items(void **state)
{
	(void)state;
	log_handler_t previous;
	void *param;
	struct captured_log log = {0};
	base_get_log_handler(&previous, &param);
	base_set_log_handler(capture_log, &log);
	enum obs_scale_type scale = obs_sceneitem_get_scale_filter(NULL);
	enum obs_blending_method method = obs_sceneitem_get_blending_method(NULL);
	enum obs_blending_type mode = obs_sceneitem_get_blending_mode(NULL);
	base_set_log_handler(previous, param);
	assert_int_equal(scale, OBS_SCALE_DISABLE);
	assert_int_equal(method, OBS_BLEND_METHOD_DEFAULT);
	assert_int_equal(mode, OBS_BLEND_NORMAL);
	assert_int_equal(log.count, 3);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_sceneitem_get_blending_mode: Null 'item' parameter");
}

/* Actual-library characterization. Unknown enum values are returned unchanged
 * (characterized, not endorsed); the three stored fields are independent. */
static void stored_values(void **state)
{
	(void)state;
	const unsigned values[] = {0, 1, 2, 3, 4, 5, 6, 7, 17, 0x7fffffff};
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		for (size_t j = 0; j < sizeof(values) / sizeof(values[0]); j++) {
			struct obs_scene_item item = {0};
			item.scale_filter = (enum obs_scale_type)values[i];
			item.blend_method = (enum obs_blending_method)values[j];
			item.blend_type = (enum obs_blending_type)values[(i + j) % 10];
			item.id = -17;
			item.removed = true;
			item.selected = true;
			unsigned char before[sizeof(item)];
			memcpy(before, &item, sizeof(item));
			assert_int_equal(obs_sceneitem_get_scale_filter(&item), item.scale_filter);
			assert_int_equal(obs_sceneitem_get_blending_method(&item), item.blend_method);
			assert_int_equal(obs_sceneitem_get_blending_mode(&item), item.blend_type);
			assert_memory_equal(before, &item, sizeof(item));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_items), cmocka_unit_test(stored_values)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

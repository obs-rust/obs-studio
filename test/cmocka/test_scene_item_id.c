#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
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
	int64_t id = obs_sceneitem_get_id(NULL);
	base_set_log_handler(previous, param);
	assert_int_equal(id, 0);
	assert_int_equal(log.count, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_sceneitem_get_id: Null 'item' parameter");
}

static void stored_ids(void **state)
{
	(void)state;
	const int64_t ids[] = {INT64_MIN, INT64_MIN + 1, -17, -1, 0, 1, 17, INT64_MAX - 1, INT64_MAX};
	for (unsigned bits = 0; bits < 32; bits++) {
		for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++) {
			struct obs_scene_item item = {0};
			item.id = ids[i];
			item.selected = (bits & 1) != 0;
			item.locked = (bits & 2) != 0;
			item.removed = (bits & 4) != 0;
			item.is_group = (bits & 8) != 0;
			item.is_scene = (bits & 16) != 0;
			unsigned char before[sizeof(item)];
			memcpy(before, &item, sizeof(item));
			assert_true(obs_sceneitem_get_id(&item) == ids[i]);
			assert_memory_equal(before, &item, sizeof(item));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_item), cmocka_unit_test(stored_ids)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
